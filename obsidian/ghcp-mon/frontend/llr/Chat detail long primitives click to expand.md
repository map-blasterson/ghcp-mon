---
type: LLR
tags:
  - req/llr
  - domain/input-breakdown
---
A primitive value rendered inside a tree node whose stringified length exceeds `200` characters or contains a newline MUST be rendered with the `ib-prim-v-clip` class and toggled open/closed by clicking it.

## Rationale
Long content should not blow up the tree by default but must be inspectable in place.

## Derived from
- [[Chat detail]]
