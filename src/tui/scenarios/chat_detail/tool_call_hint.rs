//! Tool-call follow: locate the `tool`-role message and the ancestor set
//! required to make it visible. The chat-detail render loop snaps
//! `state.focus_row` onto the returned target NodeId when a new
//! `selected_tool_call_id` arrives, so the unified focus marker (yellow `▶`)
//! ends up on that row and the bar hover indicator follows along.
//!
//! Source for (shared `frontend/llr/`):
//! - `Chat detail tool-call hint auto-expand and arrow`

use std::collections::HashSet;

use crate::tui::scenarios::chat_detail::messages::Part;
use crate::tui::scenarios::chat_detail::tree::{NodeId, NodeKind, TreeNode};

/// Locate the `tool`-role input message whose `tool_call_response` part has
/// `id == tool_call_id`. On a hit, return `(ancestors_to_expand, target_id)`
/// where `target_id` is the message node id the chat-detail render loop will
/// snap `state.focus_row` onto, and `ancestors_to_expand` is the set of node
/// ids that must be added to `expanded` to surface that row (root,
/// root/input, root/input/input_messages, and the matching message node
/// itself).
pub fn auto_expand_for_tool_call(
    root: &TreeNode,
    tool_call_id: &str,
) -> Option<(HashSet<NodeId>, NodeId)> {
    if tool_call_id.is_empty() {
        return None;
    }
    // The input section is the third child of root (system / tools / input /
    // output). Walk only that subtree.
    let input_node = root.children.iter().find(|c| c.id.as_str() == "root/input")?;
    let messages: Vec<&TreeNode> = input_node
        .children
        .iter()
        .filter(|c| matches!(c.kind, NodeKind::Message { .. }))
        .collect();
    for m in messages {
        let is_tool_role = matches!(&m.kind, NodeKind::Message { role, .. } if role == "tool");
        if !is_tool_role {
            continue;
        }
        for p in &m.children {
            if let NodeKind::Part(Part::ToolCallResponse { id, .. }) = &p.kind {
                if id == tool_call_id {
                    let mut set = HashSet::new();
                    set.insert(NodeId::from("root"));
                    set.insert(NodeId::from("root/input"));
                    set.insert(NodeId::from("root/input/input_messages"));
                    set.insert(m.id.clone());
                    return Some((set, m.id.clone()));
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::scenarios::chat_detail::tree::{build_tree, ChatContent, ChatMode};
    use serde_json::json;

    #[test]
    fn finds_tool_role_message_with_matching_response_id() {
        let attrs = json!({
            "gen_ai.input.messages": [
                {"role":"user","parts":[{"type":"text","content":"hi"}]},
                {"role":"tool","parts":[
                    {"type":"tool_call_response","id":"call_42","result":"ok"}
                ]}
            ]
        });
        let c = ChatContent::from_attrs(&attrs);
        let t = build_tree(&c, None, ChatMode::Full);
        let (set, target) = auto_expand_for_tool_call(&t, "call_42").unwrap();
        assert_eq!(target.as_str(), "root/input/input_messages/1");
        assert!(set.contains(&NodeId::from("root")));
        assert!(set.contains(&NodeId::from("root/input")));
        assert!(set.contains(&target));
    }

    #[test]
    fn ignores_non_tool_role_messages() {
        let attrs = json!({
            "gen_ai.input.messages": [
                {"role":"assistant","parts":[
                    {"type":"tool_call_response","id":"call_42","result":"ok"}
                ]}
            ]
        });
        let c = ChatContent::from_attrs(&attrs);
        let t = build_tree(&c, None, ChatMode::Full);
        assert!(auto_expand_for_tool_call(&t, "call_42").is_none());
    }

    #[test]
    fn returns_none_for_empty_id() {
        let c = ChatContent::default();
        let t = build_tree(&c, None, ChatMode::Full);
        assert!(auto_expand_for_tool_call(&t, "").is_none());
    }
}
