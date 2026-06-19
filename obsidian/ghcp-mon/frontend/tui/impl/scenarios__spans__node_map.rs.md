---
type: impl
source: src/tui/scenarios/spans/node_map.rs
lang: rust
tags:
  - impl/generated
  - impl/rust
---
Build a `HashMap<String, NodePath>` once per loaded tree change for O(1) span lookup by `span_id`. Stores index-paths into the original tree so the map can be rebuilt without cloning every node; `NodePath::resolve` walks `tree[i0].children[i1]...` to return the borrowed node.

## Source For
- [[Spans nodeMap provides O1 span lookup]]
