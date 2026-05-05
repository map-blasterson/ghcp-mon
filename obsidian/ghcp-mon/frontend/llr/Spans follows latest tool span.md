---
type: LLR
tags:
  - req/llr
  - domain/traces
---
`SpansScenario` MUST maintain an explicit follow-mode toggle (checkbox in the header). When follow-mode is enabled, the component SHALL auto-advance `selected_span_id` to the latest tool span (kind `execute_tool` or `external_tool`) as determined by `sortKey` (preferring `end_unix_ns`, falling back to `start_unix_ns`, then `span_pk`). Follow-mode MUST engage automatically when the user selects the span that is currently the latest tool span, and MUST disengage when the user manually selects any other span.

## Rationale
"Follow-tail" behaviour for live tool monitoring without trapping the user when they manually scroll away.

## Derived from
- [[Trace and Span Explorer]]
