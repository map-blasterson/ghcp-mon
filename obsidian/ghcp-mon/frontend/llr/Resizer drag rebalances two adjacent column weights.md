---
type: LLR
tags:
  - req/llr
  - domain/workspace
---
While the user drags an inter-column resizer, the workspace MUST update both adjacent columns' `width` such that their pixel sum stays equal to its pre-drag value, the left pixel width stays in `[MIN_COL_PX, total - MIN_COL_PX]`, and each column's new weight equals `totalWeight * (newPx / totalPx)`.

## Rationale
Resize is a local rebalance — other columns remain unaffected.

## Derived from
- [[Workspace Layout]]
