//! Auto-expand walk for the column's `search_query`.
//!
//! Walks the tree and collects every node ID whose ACTIVE content contains
//! `query` case-insensitively. In FULL mode "active" = all content; in DELTA
//! mode "active" = only `DiffSegment::Added` content. For every match the
//! ancestors are added to the result set so the renderer can union them
//! into `expanded` without removing user-expanded entries.
//!
//! Source for (shared `frontend/llr/`):
//! - `ChatDetail auto-expands tree to span search matches`

use std::collections::HashSet;

use serde_json::Value;

use crate::tui::scenarios::chat_detail::diff_segments::DiffSegment;
use crate::tui::scenarios::chat_detail::messages::Part;
use crate::tui::scenarios::chat_detail::tree::{ChatMode, NodeId, NodeKind, TreeNode};

/// Collect ancestor IDs whose subtree contains a match. The matching node
/// itself is NOT included (its ancestors are — that's enough to make the
/// match visible). The root's ancestors set is empty by definition.
pub fn search_expanded(root: &TreeNode, query: &str, mode: ChatMode) -> HashSet<NodeId> {
    let mut out = HashSet::new();
    if query.is_empty() {
        return out;
    }
    let needle = query.to_lowercase();
    walk(root, &needle, mode, &mut Vec::new(), &mut out);
    out
}

fn walk(
    node: &TreeNode,
    needle: &str,
    mode: ChatMode,
    ancestors: &mut Vec<NodeId>,
    out: &mut HashSet<NodeId>,
) {
    if node_matches(node, needle, mode) {
        for a in ancestors.iter() {
            out.insert(a.clone());
        }
    }
    ancestors.push(node.id.clone());
    for c in &node.children {
        walk(c, needle, mode, ancestors, out);
    }
    ancestors.pop();
}

fn node_matches(node: &TreeNode, needle: &str, mode: ChatMode) -> bool {
    if node.label.to_lowercase().contains(needle) {
        return true;
    }
    if let Some(m) = &node.meta {
        if m.to_lowercase().contains(needle) {
            return true;
        }
    }
    // Primitives (key=value rows).
    for (k, v) in &node.primitives {
        if k.to_lowercase().contains(needle) {
            return true;
        }
        if value_contains(v, needle) {
            return true;
        }
    }
    // Kind-specific content.
    match &node.kind {
        NodeKind::SystemDiff(segs) => match mode {
            ChatMode::Delta => segs.iter().any(|s| match s {
                DiffSegment::Added(t) => t.to_lowercase().contains(needle),
                _ => false,
            }),
            ChatMode::Full => segs.iter().any(|s| match s {
                DiffSegment::Unchanged(t) | DiffSegment::Added(t) | DiffSegment::Removed(t) => {
                    t.to_lowercase().contains(needle)
                }
            }),
        },
        NodeKind::Part(p) => match p {
            Part::Text { content, .. } | Part::Reasoning { content, .. } => {
                content.to_lowercase().contains(needle)
            }
            Part::ToolCall { name, id, arguments, .. } => {
                name.to_lowercase().contains(needle)
                    || id.to_lowercase().contains(needle)
                    || value_contains(arguments, needle)
            }
            Part::ToolCallResponse { id, result, .. } => {
                id.to_lowercase().contains(needle) || value_contains(result, needle)
            }
            Part::Other { raw } => value_contains(raw, needle),
        },
        _ => false,
    }
}

fn value_contains(v: &Value, needle: &str) -> bool {
    match v {
        Value::String(s) => s.to_lowercase().contains(needle),
        Value::Array(a) => a.iter().any(|x| value_contains(x, needle)),
        Value::Object(o) => o
            .iter()
            .any(|(k, x)| k.to_lowercase().contains(needle) || value_contains(x, needle)),
        Value::Number(n) => n.to_string().contains(needle),
        Value::Bool(b) => b.to_string().contains(needle),
        Value::Null => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::scenarios::chat_detail::tree::{build_tree, ChatContent};
    use serde_json::json;

    #[test]
    fn empty_query_returns_empty() {
        let c = ChatContent::default();
        let t = build_tree(&c, None, ChatMode::Full);
        assert!(search_expanded(&t, "", ChatMode::Full).is_empty());
    }

    #[test]
    fn full_mode_match_in_part_returns_ancestor_chain() {
        let attrs = json!({
            "gen_ai.input.messages": [
                {"role":"user","parts":[{"type":"text","content":"hello deepneedle"}]}
            ]
        });
        let c = ChatContent::from_attrs(&attrs);
        let t = build_tree(&c, None, ChatMode::Full);
        let s = search_expanded(&t, "DEEPNEEDLE", ChatMode::Full);
        assert!(s.contains(&NodeId::from("root")));
        assert!(s.contains(&NodeId::from("root/input")));
        assert!(s.contains(&NodeId::from("root/input/input_messages/0")));
    }

    #[test]
    fn delta_mode_only_matches_added_segments() {
        // Build a SystemChanged subtree manually via build_system_node DELTA
        let prior = ChatContent {
            system: vec![json!({"type":"text","content":"hello world"})],
            ..Default::default()
        };
        let cur = ChatContent {
            system: vec![json!({"type":"text","content":"hello brandnew"})],
            ..Default::default()
        };
        let t = build_tree(&cur, Some(&prior), ChatMode::Delta);
        // "brandnew" appears only as Added; "world" only as Removed.
        let s_add = search_expanded(&t, "brandnew", ChatMode::Delta);
        assert!(!s_add.is_empty());
        let s_rem = search_expanded(&t, "world", ChatMode::Delta);
        // In DELTA, removed-only content should NOT trigger expansion.
        // (May still match meta/labels — but "world" isn't in any of those.)
        assert!(s_rem.is_empty(), "DELTA must not match removed-only content, got: {:?}", s_rem);
    }
}
