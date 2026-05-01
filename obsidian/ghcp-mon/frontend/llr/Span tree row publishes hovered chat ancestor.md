---
type: LLR
tags:
  - req/llr
  - domain/traces
---
On `mouseenter` over a span tree row, `SpansScenario` MUST publish via `useHoverState.setHoveredChatPk` the `span_pk` of the row's nearest chat ancestor (or the row itself if it is a chat span), and `null` for rows with no chat ancestor; on `mouseleave` it MUST publish `null`.

## Rationale
This drives the cross-component highlight in the Context Growth widget.

## Derived from
- [[Trace and Span Explorer]]
