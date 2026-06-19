//! Auto-expand walks for the column's `search_query`.
//!
//! Two complementary walks:
//!  * [`search_expanded`] collects every node ID whose ACTIVE content
//!    contains `query` case-insensitively, PLUS every ancestor on the way
//!    down. Including the matching node itself (a change from the original
//!    "ancestors only" contract) means rows like a `Part::Text` whose body
//!    contains the match auto-open to reveal the matching line.
//!  * [`search_expanded_prims`] returns the `(node_id, primitive_index)`
//!    pairs whose key or value contains the needle, so long primitives
//!    auto-open from their truncated `(+)` state to show the matched text.
//!
//! In FULL mode "active" = all content; in DELTA mode "active" = only
//! `DiffSegment::Added` content (so unchanged baseline content does not
//! trigger expansion).
//!
//! Source for (shared `frontend/llr/`):
//! - `ChatDetail auto-expands tree to span search matches`

use std::collections::HashSet;

use serde_json::Value;

use crate::tui::scenarios::chat_detail::diff_segments::DiffSegment;
use crate::tui::scenarios::chat_detail::messages::Part;
use crate::tui::scenarios::chat_detail::tree::{ChatMode, NodeId, NodeKind, TreeNode};

/// Collect the IDs of every node containing a match, plus every ancestor on
/// the path to that node. The matching node itself IS included so rows with
/// collapsible bodies (Parts, SystemChanged) expand to reveal the match. The
/// root's ancestors set is empty by definition.
pub fn search_expanded(root: &TreeNode, query: &str, mode: ChatMode) -> HashSet<NodeId> {
    let mut out = HashSet::new();
    if query.is_empty() {
        return out;
    }
    let needle = query.to_lowercase();
    walk(root, &needle, mode, &mut Vec::new(), &mut out);
    out
}

/// Collect `(node_id, primitive_index)` pairs whose key or value contains
/// `query` case-insensitively. The chat-detail renderer treats this set as
/// the auto-open analogue of [`search_expanded`] for the primitive layer:
/// long primitives that would otherwise render truncated with a `(+)`
/// indicator open to show the matched content.
pub fn search_expanded_prims(root: &TreeNode, query: &str) -> HashSet<(NodeId, usize)> {
    let mut out = HashSet::new();
    if query.is_empty() {
        return out;
    }
    let needle = query.to_lowercase();
    walk_prims(root, &needle, &mut out);
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
        // The matching node itself is added too — rows with collapsible
        // bodies need to open to show the actual matched content.
        out.insert(node.id.clone());
    }
    ancestors.push(node.id.clone());
    for c in &node.children {
        walk(c, needle, mode, ancestors, out);
    }
    ancestors.pop();
}

fn walk_prims(node: &TreeNode, needle: &str, out: &mut HashSet<(NodeId, usize)>) {
    for (i, (k, v)) in node.primitives.iter().enumerate() {
        if k.to_lowercase().contains(needle) || value_contains(v, needle) {
            out.insert((node.id.clone(), i));
        }
    }
    for c in &node.children {
        walk_prims(c, needle, out);
    }
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

    #[test]
    fn matching_node_id_itself_is_included_so_part_bodies_expand() {
        // Reproduce the gap that motivated this change: a match inside a
        // Part::Text body. Without including the matching node itself, the
        // Part row was visible but stayed collapsed → matched line hidden.
        let attrs = json!({
            "gen_ai.input.messages": [
                {"role":"user","parts":[{"type":"text","content":"hello deepneedle"}]}
            ]
        });
        let c = ChatContent::from_attrs(&attrs);
        let t = build_tree(&c, None, ChatMode::Full);
        let s = search_expanded(&t, "deepneedle", ChatMode::Full);
        // Locate the part node id by walking the built tree.
        let part_id = find_part_id_under(&t, "root/input/input_messages/0")
            .expect("a Part child should exist under the user message");
        assert!(
            s.contains(&part_id),
            "expected the Part node id ({:?}) to be in the auto-expand set so its body renders, got {:?}",
            part_id,
            s,
        );
    }

    fn find_part_id_under(root: &TreeNode, parent_id: &str) -> Option<NodeId> {
        fn walk<'a>(n: &'a TreeNode, parent_id: &str) -> Option<&'a TreeNode> {
            if n.id.as_str() == parent_id {
                return Some(n);
            }
            for c in &n.children {
                if let Some(found) = walk(c, parent_id) {
                    return Some(found);
                }
            }
            None
        }
        let parent = walk(root, parent_id)?;
        parent
            .children
            .iter()
            .find(|c| matches!(c.kind, NodeKind::Part(_)))
            .map(|c| c.id.clone())
    }

    #[test]
    fn search_expanded_prims_finds_matches_in_primitive_values() {
        // Build a tree containing a tool-call part whose `arguments` JSON
        // includes a long string — that string lives in the parent
        // ToolCall node's primitives (or in the message's primitives,
        // depending on how the tree builder lays it out). We use a value
        // string that's guaranteed unique and verify the prim set is
        // non-empty so the renderer auto-opens the truncated row.
        let attrs = json!({
            "gen_ai.input.messages": [
                {"role":"assistant","parts":[{
                    "type":"tool_call",
                    "id":"call_0",
                    "name":"bash",
                    "arguments":{"cmd":"echo UNIQUE_NEEDLE_TOKEN_XYZ"}
                }]}
            ]
        });
        let c = ChatContent::from_attrs(&attrs);
        let t = build_tree(&c, None, ChatMode::Full);
        let pset = search_expanded_prims(&t, "UNIQUE_NEEDLE_TOKEN_XYZ");
        assert!(
            !pset.is_empty(),
            "expected at least one primitive match for the unique needle, got: {:?}",
            pset,
        );
        // Empty query returns empty set.
        assert!(search_expanded_prims(&t, "").is_empty());
    }
}
