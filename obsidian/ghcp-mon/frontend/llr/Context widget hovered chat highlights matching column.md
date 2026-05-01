---
type: LLR
tags:
  - req/llr
  - domain/context-growth
---
The chart's stacked column for chat span_pk `p` MUST receive the `hovered` class iff `useHoverState().hoveredChatPk === p`.

## Rationale
Cross-component highlight: hovering a span tree row in the Spans column lights up the matching context bar in the widget.

## Derived from
- [[Context Growth Widget]]
