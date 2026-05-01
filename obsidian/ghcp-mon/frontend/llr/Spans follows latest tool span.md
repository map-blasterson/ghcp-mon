---
type: LLR
tags:
  - req/llr
  - domain/traces
---
While the user's `selected_span_id` matches the previously-latest tool span (kind `execute_tool` or `external_tool`) in the session tree, `SpansScenario` SHOULD auto-advance the selection to the new latest tool span when one arrives, and MUST stop auto-advancing as soon as `selected_span_id` differs from the previous latest.

## Rationale
"Follow-tail" behaviour for live tool monitoring without trapping the user when they manually scroll away.

## Derived from
- [[Trace and Span Explorer]]
