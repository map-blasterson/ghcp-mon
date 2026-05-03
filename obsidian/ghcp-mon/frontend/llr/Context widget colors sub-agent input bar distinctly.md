---
type: LLR
tags:
  - req/llr
  - domain/context-growth
---
In the Context Growth Widget chart, the `input` sub-bar of sub-agent chat columns (i.e., chat snapshots whose `invoke_agent` ancestor depth is greater than 1) MUST be rendered in a visually distinct color from the root agent's `input` sub-bar — specifically, a lighter shade of the same blue. The `output` and `reasoning` sub-bars MUST use the same colors as the root agent. No background tint, gap, or other column-level decoration is required to mark sub-agent columns.

## Rationale
Sub-agent chat turns share the chart with the root agent's turns ([[Context widget includes sub-agent chats]]) and need a quick visual cue so users can tell them apart at a glance. Recoloring just the `input` sub-bar — which is the largest stack segment for chat turns — keeps the cue consistent with the input/output/reasoning palette while staying out of the way of the bar geometry, hover affordance, and y-axis scale.

## Derived from
- [[Context Growth Widget]]
