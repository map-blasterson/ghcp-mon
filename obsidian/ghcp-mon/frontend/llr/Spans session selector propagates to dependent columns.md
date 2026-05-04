---
type: LLR
tags:
  - req/llr
  - domain/traces
---
When the user changes the session dropdown in the `SpansScenario` column header, the component SHALL update its own `config.session` and clear `selected_trace_id` and `selected_span_id`, AND SHALL propagate the new session to every sibling column whose `scenarioType` is `"spans"`, `"chat_detail"`, or `"file_touches"` by writing their `config.session`.

## Rationale
Mirrors the propagation behavior of `LiveSessionsScenario.onSelect` so that session switching from the Spans column keeps all session-scoped columns in sync, regardless of which column initiated the change.

## Derived from
- [[Trace and Span Explorer]]
