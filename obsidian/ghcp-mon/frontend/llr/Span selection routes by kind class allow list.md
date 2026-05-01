---
type: LLR
tags:
  - req/llr
  - domain/traces
---
When a span is picked in the spans column, `SpansScenario` MUST update each other column according to the kind allow-list `{ spans: "*", tool_detail: ["execute_tool", "external_tool"], chat_detail: ["chat"] }`: a target column MUST receive the new `selected_trace_id` / `selected_span_id` only if its scenario type is in the map and either the entry is `"*"` or it includes the picked `kind_class`.

## Rationale
Non-applicable scenarios keep their last applicable selection so the user can keep multiple inspectors live at once.

## Derived from
- [[Trace and Span Explorer]]
