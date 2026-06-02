//! Edit/write tool renderer — path + old/new blocks (syntect-highlighted by the
//! file extension) with the remaining args as JSON, and a result section that
//! prefers a unified diff (`metadata.diff`) over `output` over JSON. Mirrors the
//! web `EditArgs` / `renderEditResult` (minus the word-level `InlineDiff`, which
//! is deliberately deferred to Phase 4).
//!
//! Source for (shared `frontend/llr/`):
//! - `Edit tool renders old new with syntax highlight`
//! - `Edit tool result renders unified diff from metadata`
//!
//! Also source for (new `frontend/tui/llr/`):
//! - `TUI Udiff classify line precedence`

use ratatui::style::{Color, Modifier, Style};
use serde_json::Value;

use super::content::parse_tool_call_result;
use super::udiff::{udiff_classify, UdiffLine};
use super::BodyCtx;
use crate::tui::scenarios::spans::attrs::parse_tool_call_arguments;
use crate::tui::widgets::lang_from_path::lang_from_path;

/// The chosen result-rendering strategy for an `edit`/`write` tool result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EditResult {
    /// Result was a plain string → render verbatim.
    Body(String),
    /// `metadata.diff` non-empty string → render colored unified diff.
    Diff(String),
    /// `output` non-empty string → render verbatim.
    Output(String),
    /// Anything else → JSON pretty-print.
    Json,
}

/// Classify how a result value should render. Precedence:
/// string body → `metadata.diff` → `output` → JSON. Mirrors web
/// `renderEditResult`.
pub fn classify_edit_result(result: &Value) -> EditResult {
    if let Value::String(s) = result {
        return EditResult::Body(s.clone());
    }
    if let Some(obj) = result.as_object() {
        if let Some(meta) = obj.get("metadata").and_then(|m| m.as_object()) {
            if let Some(Value::String(d)) = meta.get("diff") {
                if !d.is_empty() {
                    return EditResult::Diff(d.clone());
                }
            }
        }
        if let Some(Value::String(o)) = obj.get("output") {
            if !o.is_empty() {
                return EditResult::Output(o.clone());
            }
        }
    }
    EditResult::Json
}

/// Per-byte base styles coloring a unified-diff blob by line class.
pub(crate) fn udiff_byte_styles(text: &str) -> Vec<Style> {
    let mut styles = Vec::with_capacity(text.len());
    for line in text.split_inclusive('\n') {
        let trimmed = line.strip_suffix('\n').unwrap_or(line);
        let style = match udiff_classify(trimmed) {
            UdiffLine::Meta => Style::default().fg(Color::Cyan).add_modifier(Modifier::DIM),
            UdiffLine::Hunk => Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD),
            UdiffLine::Add => Style::default().fg(Color::Green),
            UdiffLine::Rem => Style::default().fg(Color::Red),
            UdiffLine::Line => Style::default(),
        };
        for _ in 0..line.len() {
            styles.push(style);
        }
    }
    styles
}

fn str_field<'a>(obj: &'a serde_json::Map<String, Value>, keys: &[&str]) -> Option<&'a str> {
    for k in keys {
        if let Some(Value::String(s)) = obj.get(*k) {
            return Some(s);
        }
    }
    None
}

/// Render the edit/write args + result section.
pub(crate) fn render(ctx: &mut BodyCtx, attrs: &Value) {
    let args = parse_tool_call_arguments(attrs).filter(|v| !v.is_null());
    let result = parse_tool_call_result(attrs);
    let args_obj = args.as_ref().and_then(|v| v.as_object()).cloned();

    if args_obj.is_none() && result.is_none() {
        ctx.no_content();
        return;
    }

    if let Some(obj) = &args_obj {
        ctx.sublabel("arguments");
        let path = str_field(obj, &["path", "filePath"]).map(|s| s.to_string());
        let old_str = str_field(obj, &["old_str", "oldString"]).map(|s| s.to_string());
        // Body-text sources (file_text from Copilot's `create`, content from
        // opencode's write) label the block "content"; new-text sources
        // (new_str, newString) keep the "new_str" label. Per the
        // `Edit tool renders old new with syntax highlight` LLR + Copilot
        // adapter: file-path / old-text / new-text / body-text are the
        // four normalized argument concepts; body-text is Copilot's
        // `file_text` and opencode's `content`.
        let new_text = str_field(obj, &["new_str", "newString"]);
        let body_text = str_field(obj, &["file_text", "content"]);
        let new_str = new_text.or(body_text).map(|s| s.to_string());
        let new_label = if new_text.is_some() {
            "new_str"
        } else if body_text.is_some() {
            "content"
        } else {
            "new_str"
        };
        let lang = path.as_deref().and_then(lang_from_path);

        if let Some(p) = &path {
            ctx.kv_row("path", p);
        }

        // When both `old_str` and `new_str` are present this is a diff-shape
        // update — collapse the two separate code blocks into one inline
        // line-level diff. Mirrors the web `InlineDiff` component, minus
        // word-level intra-line emphasis (deferred). When only one side is
        // present (a fresh write via `content`/`file_text`, or an
        // ill-formed call missing `new_str`) keep the single-block path.
        match (&old_str, &new_str) {
            (Some(old), Some(new)) => {
                ctx.sublabel("diff");
                let rows = super::inline_diff::build(old, new);
                let (text, styles, gutter) = super::inline_diff::render(&rows);
                ctx.search_block("edit.diff", &text, Some(styles), Some(&gutter));
            }
            (None, Some(new)) => {
                ctx.sublabel(new_label);
                ctx.code_block("edit.new", new, lang);
            }
            (Some(old), None) => {
                ctx.sublabel("old_str");
                ctx.code_block("edit.old", old, lang);
            }
            (None, None) => {}
        }

        // Remaining args under "other" as JSON.
        let mut extra = serde_json::Map::new();
        for (k, v) in obj {
            if matches!(
                k.as_str(),
                "path"
                    | "filePath"
                    | "old_str"
                    | "oldString"
                    | "new_str"
                    | "newString"
                    | "content"
                    | "file_text"
            ) {
                continue;
            }
            extra.insert(k.clone(), v.clone());
        }
        if !extra.is_empty() {
            ctx.sublabel("other");
            ctx.json_text("edit.other", &Value::Object(extra));
        }
    }

    if let Some(result) = &result {
        ctx.gap();
        ctx.sublabel("result");
        match classify_edit_result(result) {
            EditResult::Body(s) | EditResult::Output(s) => {
                ctx.search_block("edit.result", &s, None, None)
            }
            EditResult::Diff(d) => {
                let styles = udiff_byte_styles(&d);
                ctx.search_block("edit.result", &d, Some(styles), None);
            }
            EditResult::Json => ctx.json_text("edit.result", result),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn string_result_is_body() {
        assert_eq!(
            classify_edit_result(&json!("done")),
            EditResult::Body("done".to_string())
        );
    }

    #[test]
    fn metadata_diff_wins_over_output() {
        let r = json!({"output": "Wrote", "metadata": {"diff": "@@ -1 +1 @@"}});
        assert_eq!(
            classify_edit_result(&r),
            EditResult::Diff("@@ -1 +1 @@".to_string())
        );
    }

    #[test]
    fn output_used_when_no_diff() {
        let r = json!({"output": "Wrote file successfully.", "metadata": {}});
        assert_eq!(
            classify_edit_result(&r),
            EditResult::Output("Wrote file successfully.".to_string())
        );
    }

    #[test]
    fn empty_diff_falls_through_to_output() {
        let r = json!({"output": "ok", "metadata": {"diff": ""}});
        assert_eq!(classify_edit_result(&r), EditResult::Output("ok".to_string()));
    }

    #[test]
    fn structured_without_envelope_is_json() {
        assert_eq!(classify_edit_result(&json!({"a": 1})), EditResult::Json);
    }

    #[test]
    fn udiff_styles_color_add_and_rem() {
        let styles = udiff_byte_styles("+added\n-removed\n");
        assert_eq!(styles[0].fg, Some(Color::Green));
        let rem_off = "+added\n".len();
        assert_eq!(styles[rem_off].fg, Some(Color::Red));
    }
}
