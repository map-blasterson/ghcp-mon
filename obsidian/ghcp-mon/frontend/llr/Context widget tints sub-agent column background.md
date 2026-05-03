---
type: LLR
tags:
  - req/llr
  - domain/context-growth
---
In the Context Growth Widget chart, the columns occupied by sub-agent chat turns (i.e., chat snapshots whose `invoke_agent` ancestor depth is greater than 1) MUST be visually distinguished from top-level chat columns by tinting the column's background with a lighter color than the chart background, with the tint extending at least 3px beyond each side of the column. Additionally, at every transition between a sub-agent column and a parent-agent column, the chart MUST insert at least 6px of horizontal padding so the boundary is visible. Sub-agent bars themselves MUST use the same color palette as top-level bars.

## Rationale
Now that [[Context widget includes sub-agent chats]] surfaces sub-agent chat turns as their own rows, users need a quick visual cue to distinguish sub-agent context usage from top-level chat context usage without having to inspect each row. Tinting the column background — and inserting a clear gap at group boundaries — keeps the input / output / reasoning palette consistent across all bars while still calling out sub-agent provenance. Whole-pixel sizes (rather than sub-pixel like 1.5px) ensure the cues remain visible at 100% browser zoom.

## Derived from
- [[Context Growth Widget]]
