---
type: LLR
tags:
  - req/llr
  - domain/traces
---
`SpanDetailView` MUST render every projection sub-block present on `detail.projection` — `chat_turn`, `tool_call`, `agent_run`, `external_tool_call` — each inside an open `<details>` block whose body is a `JsonView` of the projection record, and MUST omit the projection section entirely when `projection` is empty.

## Rationale
The user inspects the canonical span plus the scenario-specific projections in one place.

## Derived from
- [[Trace and Span Explorer]]
