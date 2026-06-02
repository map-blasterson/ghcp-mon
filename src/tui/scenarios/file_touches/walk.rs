//! Pure "extract touches from a session span tree" helper for File Touches.
//!
//! Walks a `Vec<SpanNode>` recursively and, for each `execute_tool` span whose
//! Copilot-normalized tool kind is one of `read`/`write`/`edit`/`patch`, reads
//! the captured `gen_ai.tool.call.arguments` and extracts the touched file
//! path(s) **through the vendor module** (never hardcoded argument names — see
//! the Phase 3 `b44e4ca` vendor-arg-name bug-fix lesson).
//!
//! Source for (shared `frontend/llr/`):
//! - `File touches aggregates view edit create`
//! - `File touches builds filesystem tree with counts`

use crate::tui::model::{KindClass, SpanDetail, SpanNode};
use crate::tui::scenarios::file_touches::tree::{Touch, TouchKind, TouchRef};
use crate::tui::scenarios::spans::attrs::parse_tool_call_arguments;
use crate::tui::vendor::copilot::{self, ArgConcept, ToolKind};

/// Walk a session span tree and return one [`Touch`] per extracted file path.
///
/// `detail_lookup` resolves a span's full detail (the cached
/// `["span", trace_id, span_id]` entry). When it returns `None` the span is
/// simply skipped — the next render tick re-runs after the cache populates.
pub fn extract_touches(
    tree: &[SpanNode],
    mut detail_lookup: impl FnMut(&str, &str) -> Option<SpanDetail>,
) -> Vec<Touch> {
    let mut out = Vec::new();
    for node in tree {
        walk_node(node, &mut detail_lookup, &mut out);
    }
    out
}

fn walk_node(
    node: &SpanNode,
    detail_lookup: &mut impl FnMut(&str, &str) -> Option<SpanDetail>,
    out: &mut Vec<Touch>,
) {
    if node.kind_class == KindClass::ExecuteTool {
        if let Some(detail) = detail_lookup(&node.trace_id, &node.span_id) {
            collect_from_detail(&detail, out);
        }
    }
    // Recurse unconditionally — tool spans nest under invoke_agent spans.
    for child in &node.children {
        walk_node(child, detail_lookup, out);
    }
}

fn collect_from_detail(detail: &SpanDetail, out: &mut Vec<Touch>) {
    // Normalized tool kind from the projection's tool name.
    let tool_name = detail
        .projection
        .tool_call
        .as_ref()
        .and_then(|t| t.tool_name.as_deref());
    let Some(kind) = tool_name.and_then(copilot::tool_name_mapping) else {
        return;
    };
    // Only the four file-touching kinds participate.
    let touch_kind = match kind {
        ToolKind::Read => TouchKind::Read,
        ToolKind::Write | ToolKind::Edit | ToolKind::Patch => TouchKind::Write,
        ToolKind::Shell => return,
    };

    let Some(attrs) = detail.span.attributes.as_ref() else {
        return;
    };
    let Some(args) = parse_tool_call_arguments(attrs) else {
        return;
    };

    let span_ref = TouchRef {
        span_id: detail.span.span_id.clone(),
        trace_id: detail.span.trace_id.clone(),
        span_pk: detail.span.span_pk,
    };

    match kind {
        ToolKind::Patch => {
            // A patch may touch multiple files; the vendor module parses the
            // patch text. Every extracted path is a write.
            for path in copilot::extract_apply_patch_paths(&args) {
                out.push(Touch {
                    path,
                    kind: TouchKind::Write,
                    span_ref: span_ref.clone(),
                });
            }
        }
        ToolKind::Read | ToolKind::Write | ToolKind::Edit => {
            // Resolve the normalized file-path argument via the vendor adapter
            // (Copilot: `args.path`). Skip silently if absent or non-string.
            if let Some(p) = copilot::resolve_argument(&args, ArgConcept::FilePath)
                .and_then(|v| v.as_str())
            {
                out.push(Touch {
                    path: p.to_string(),
                    kind: touch_kind,
                    span_ref,
                });
            }
        }
        ToolKind::Shell => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::model::{
        SpanFull, SpanProjection, ToolCallProjection,
    };
    use serde_json::{json, Value};

    fn node(span_id: &str, kind_class: KindClass, children: Vec<SpanNode>) -> SpanNode {
        SpanNode {
            span_pk: 1,
            trace_id: "t".into(),
            span_id: span_id.into(),
            parent_span_id: None,
            name: "n".into(),
            kind_class,
            ingestion_state: "real".into(),
            start_unix_ns: Some(1),
            end_unix_ns: Some(2),
            projection: SpanProjection::default(),
            children,
        }
    }

    fn detail(tool_name: &str, args: Value) -> SpanDetail {
        let attrs = json!({ "gen_ai.tool.call.arguments": args });
        SpanDetail {
            span: SpanFull {
                span_pk: 7,
                trace_id: "t".into(),
                span_id: "s".into(),
                parent_span_id: None,
                name: "n".into(),
                kind: Some(5),
                kind_class: KindClass::ExecuteTool,
                start_unix_ns: Some(1),
                end_unix_ns: Some(2),
                duration_ns: Some(1),
                status_message: None,
                ingestion_state: "real".into(),
                scope_name: None,
                scope_version: None,
                attributes: Some(attrs),
                resource: None,
            },
            events: Vec::new(),
            parent: None,
            children: Vec::new(),
            projection: SpanProjection {
                tool_call: Some(ToolCallProjection {
                    tool_name: Some(tool_name.into()),
                    ..Default::default()
                }),
                ..Default::default()
            },
        }
    }

    /// A lookup that returns a fixed detail for any (trace, span).
    fn lookup_const(d: SpanDetail) -> impl Fn(&str, &str) -> Option<SpanDetail> {
        move |_, _| Some(d.clone())
    }

    #[test]
    fn single_read_span_yields_one_read_touch() {
        let tree = vec![node("s", KindClass::ExecuteTool, vec![])];
        let d = detail("view", json!({ "path": "src/main.rs" }));
        let touches = extract_touches(&tree, lookup_const(d));
        assert_eq!(touches.len(), 1);
        assert_eq!(touches[0].path, "src/main.rs");
        assert_eq!(touches[0].kind, TouchKind::Read);
    }

    #[test]
    fn single_edit_span_yields_one_write_touch() {
        let tree = vec![node("s", KindClass::ExecuteTool, vec![])];
        let d = detail(
            "edit",
            json!({ "path": "a.rs", "old_str": "x", "new_str": "y" }),
        );
        let touches = extract_touches(&tree, lookup_const(d));
        assert_eq!(touches.len(), 1);
        assert_eq!(touches[0].kind, TouchKind::Write);
    }

    #[test]
    fn copilot_create_span_extracts_write_touch_with_path() {
        // Regression for the vendor-arg-name pitfall (b44e4ca): `create` maps
        // to Write and its args carry `path` alongside `file_text`. The
        // file-path concept must resolve through the vendor adapter.
        let tree = vec![node("s", KindClass::ExecuteTool, vec![])];
        let d = detail(
            "create",
            json!({ "path": "new/file.rs", "file_text": "fn main() {}" }),
        );
        let touches = extract_touches(&tree, lookup_const(d));
        assert_eq!(touches.len(), 1);
        assert_eq!(touches[0].path, "new/file.rs");
        assert_eq!(touches[0].kind, TouchKind::Write);
    }

    #[test]
    fn patch_span_with_three_paths_yields_three_write_touches() {
        let tree = vec![node("s", KindClass::ExecuteTool, vec![])];
        let patch = "*** Begin Patch\n\
*** Add File: a.rs\n+line\n\
*** Update File: b.rs\n+line\n\
*** Delete File: c.rs\n\
*** End Patch\n";
        let d = detail("apply_patch", json!({ "patch": patch }));
        let touches = extract_touches(&tree, lookup_const(d));
        assert_eq!(touches.len(), 3);
        assert!(touches.iter().all(|t| t.kind == TouchKind::Write));
        let paths: Vec<&str> = touches.iter().map(|t| t.path.as_str()).collect();
        assert_eq!(paths, vec!["a.rs", "b.rs", "c.rs"]);
    }

    #[test]
    fn tool_kind_outside_the_four_is_skipped() {
        let tree = vec![node("s", KindClass::ExecuteTool, vec![])];
        // bash maps to Shell — not a file touch.
        let d = detail("bash", json!({ "command": "ls" }));
        let touches = extract_touches(&tree, lookup_const(d));
        assert!(touches.is_empty());
    }

    #[test]
    fn unknown_tool_name_is_skipped() {
        let tree = vec![node("s", KindClass::ExecuteTool, vec![])];
        let d = detail("task", json!({ "path": "x.rs" }));
        let touches = extract_touches(&tree, lookup_const(d));
        assert!(touches.is_empty());
    }

    #[test]
    fn missing_arguments_attribute_is_skipped() {
        let tree = vec![node("s", KindClass::ExecuteTool, vec![])];
        // Detail with empty attributes (no gen_ai.tool.call.arguments).
        let mut d = detail("view", json!({ "path": "x.rs" }));
        d.span.attributes = Some(json!({}));
        let touches = extract_touches(&tree, lookup_const(d));
        assert!(touches.is_empty());
    }

    #[test]
    fn non_string_path_value_is_skipped() {
        let tree = vec![node("s", KindClass::ExecuteTool, vec![])];
        let d = detail("view", json!({ "path": 42 }));
        let touches = extract_touches(&tree, lookup_const(d));
        assert!(touches.is_empty());
    }

    #[test]
    fn uncached_detail_is_skipped() {
        let tree = vec![node("s", KindClass::ExecuteTool, vec![])];
        let touches = extract_touches(&tree, |_, _| None);
        assert!(touches.is_empty());
    }

    #[test]
    fn nested_tool_spans_under_invoke_agent_are_extracted() {
        // invoke_agent (not a tool) → contains an execute_tool child.
        let inner = node("tool", KindClass::ExecuteTool, vec![]);
        let agent = node("agent", KindClass::InvokeAgent, vec![inner]);
        let tree = vec![agent];
        let d = detail("view", json!({ "path": "deep/file.rs" }));
        let touches = extract_touches(&tree, lookup_const(d));
        assert_eq!(touches.len(), 1);
        assert_eq!(touches[0].path, "deep/file.rs");
    }
}
