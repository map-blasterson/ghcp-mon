//! Walk the loaded span tree once per change; expose `HashMap<String,
//! NodeRef>` for O(1) span lookup by `span_id`. Implements
//! `Spans nodeMap provides O1 span lookup`.

use std::collections::HashMap;

use crate::tui::model::SpanNode;

/// Borrowed reference into a [`SpanNode`] subtree. Uses indices into the
/// owner tree so the map can be rebuilt without cloning every node.
#[derive(Debug, Clone, Copy)]
pub struct NodePath {
    /// Index path from the tree root list (e.g. `[2, 0, 1]` means
    /// `tree[2].children[0].children[1]`).
    pub indices: [usize; 16],
    pub depth: usize,
}

impl NodePath {
    fn root(i: usize) -> Self {
        let mut a = [0; 16];
        a[0] = i;
        Self { indices: a, depth: 1 }
    }
    fn push(mut self, i: usize) -> Self {
        if self.depth < self.indices.len() {
            self.indices[self.depth] = i;
            self.depth += 1;
        }
        self
    }
    pub fn resolve<'a>(&self, tree: &'a [SpanNode]) -> Option<&'a SpanNode> {
        let mut cur = tree.get(self.indices[0])?;
        for d in 1..self.depth {
            cur = cur.children.get(self.indices[d])?;
        }
        Some(cur)
    }
}

/// `span_id` → path for O(1) lookup.
pub type NodeMap = HashMap<String, NodePath>;

/// Build a node-map by walking the entire forest.
pub fn build_from_tree(tree: &[SpanNode]) -> NodeMap {
    let mut out = NodeMap::with_capacity(tree.len() * 4);
    fn walk(node: &SpanNode, path: NodePath, out: &mut NodeMap) {
        out.insert(node.span_id.clone(), path);
        for (i, child) in node.children.iter().enumerate() {
            walk(child, path.push(i), out);
        }
    }
    for (i, root) in tree.iter().enumerate() {
        walk(root, NodePath::root(i), &mut out);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::model::{KindClass, SpanProjection};

    fn mk(id: &str, children: Vec<SpanNode>) -> SpanNode {
        SpanNode {
            span_pk: 0,
            trace_id: "t".into(),
            span_id: id.into(),
            parent_span_id: None,
            name: id.into(),
            kind_class: KindClass::Other,
            ingestion_state: "complete".into(),
            start_unix_ns: None,
            end_unix_ns: None,
            projection: SpanProjection::default(),
            children,
        }
    }

    #[test]
    fn round_trip_finds_every_node() {
        let tree = vec![
            mk("r1", vec![mk("a", vec![mk("aa", vec![])]), mk("b", vec![])]),
            mk("r2", vec![mk("c", vec![])]),
        ];
        let map = build_from_tree(&tree);
        for id in ["r1", "r2", "a", "aa", "b", "c"] {
            let path = map.get(id).expect(id);
            let node = path.resolve(&tree).expect(id);
            assert_eq!(node.span_id, id);
        }
        assert!(map.get("missing").is_none());
    }

    #[test]
    fn empty_tree_is_empty_map() {
        let map = build_from_tree(&[]);
        assert!(map.is_empty());
    }
}
