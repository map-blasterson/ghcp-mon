---
type: LLR
tags:
  - req/llr
  - domain/input-breakdown
---
The summary bar above the tree MUST render one segment per currently-visible (collapsed-frontier) node, with each segment's width set to `(seg.bytes / max(1, totalBytes)) * 100%`; hovering a tree node MUST mark the corresponding bar segment with the `hovered` class and vice versa.

## Rationale
The bar is a stacked overview that mirrors what the user has expanded.

## Derived from
- [[Chat Input Breakdown]]
