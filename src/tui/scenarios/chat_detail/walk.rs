//! Collapsed-frontier walker.
//!
//! Returns the topmost expanded node and recurses through its expanded
//! children; for each *collapsed* descendant it returns the node itself
//! (without descending). The summary bar and the tree-row renderer both
//! consume this list so the two views stay in sync.
//!
//! Source for (shared `frontend/llr/`):
//! - `Chat detail summary bar proportional to visible segments`

use std::collections::HashSet;

use crate::tui::scenarios::chat_detail::tree::{NodeId, TreeNode};

/// Walk `root` and return the collapsed-frontier node list. A node is
/// included if either:
///  * its parent was expanded AND it is not in `expanded` (collapsed leaf of
///    the visible front), OR
///  * it has no children at all.
///
/// The root itself is always considered "above the frontier" — its 4 direct
/// children are the topmost candidates the summary bar paints.
pub fn visible_frontier<'a>(
    root: &'a TreeNode,
    expanded: &HashSet<NodeId>,
) -> Vec<&'a TreeNode> {
    let mut out: Vec<&TreeNode> = Vec::new();
    // The summary bar always reports against the 4 top-level branches at
    // minimum: descend into each according to expansion.
    for child in &root.children {
        recurse(child, expanded, &mut out);
    }
    out
}

fn recurse<'a>(node: &'a TreeNode, expanded: &HashSet<NodeId>, out: &mut Vec<&'a TreeNode>) {
    if expanded.contains(&node.id) && !node.children.is_empty() {
        for c in &node.children {
            recurse(c, expanded, out);
        }
    } else {
        out.push(node);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::scenarios::chat_detail::tree::{build_tree, ChatContent, ChatMode};
    use serde_json::json;

    fn fixture_root() -> TreeNode {
        let attrs = json!({
            "gen_ai.system_instructions": [{"type":"text","content":"hi"}],
            "gen_ai.tool.definitions": [{"name":"ls"}],
            "gen_ai.input.messages": [
                {"role":"user","parts":[{"type":"text","content":"x"}]},
                {"role":"assistant","parts":[{"type":"text","content":"y"}]}
            ],
            "gen_ai.output.messages": []
        });
        let c = ChatContent::from_attrs(&attrs);
        build_tree(&c, None, ChatMode::Full)
    }

    #[test]
    fn collapsed_root_returns_four_children() {
        let root = fixture_root();
        let expanded: HashSet<NodeId> = HashSet::new();
        let frontier = visible_frontier(&root, &expanded);
        assert_eq!(frontier.len(), 4);
    }

    #[test]
    fn expanding_input_descends_into_messages() {
        let root = fixture_root();
        let mut expanded: HashSet<NodeId> = HashSet::new();
        expanded.insert("root/input".into());
        let frontier = visible_frontier(&root, &expanded);
        // 3 collapsed siblings + 2 messages under input.
        assert_eq!(frontier.len(), 5);
        assert!(frontier.iter().any(|n| n.id.as_str() == "root/input/input_messages/0"));
        assert!(frontier.iter().any(|n| n.id.as_str() == "root/input/input_messages/1"));
    }

    #[test]
    fn nested_expansion_descends_further() {
        let root = fixture_root();
        let mut expanded: HashSet<NodeId> = HashSet::new();
        expanded.insert("root/input".into());
        expanded.insert("root/input/input_messages/0".into());
        let frontier = visible_frontier(&root, &expanded);
        // 3 collapsed root siblings + 1 sibling message + 1 part.
        assert_eq!(frontier.len(), 5);
        assert!(frontier
            .iter()
            .any(|n| n.id.as_str() == "root/input/input_messages/0/parts/0"));
    }
}
