---
type: impl
source: src/tui/scenarios/chat_detail/walk.rs
lang: rust
tags:
  - impl/generated
  - impl/rust
---
Collapsed-frontier walker. `visible_frontier(root, expanded)` returns the topmost expanded node and recurses through its expanded children; for each collapsed (or leaf) node it returns the node itself without descending. The summary bar segments + the tree-row renderer both consume this list so byte proportions track the user's current expansion state.

## Source For
- [[Chat detail summary bar proportional to visible segments]]
