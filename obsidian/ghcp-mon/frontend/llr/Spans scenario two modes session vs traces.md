---
type: LLR
tags:
  - req/llr
  - domain/traces
---
`SpansScenario` MUST render the `/api/sessions/:cid/span-tree` response as an expandable tree when `column.config.session` is set, and MUST render the `/api/traces` list when it is not. Switching `session` MUST clear the column's `selected_trace_id` and `selected_span_id`.

## Rationale
The two modes serve different points in a trace's lifecycle: list new traces before any chat span pins them to a session, then drill into the selected session.

## Derived from
- [[Trace and Span Explorer]]
- [[API session span tree trace scoped union]]
