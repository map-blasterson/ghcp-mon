---
type: LLR
tags:
  - req/llr
  - domain/traces
---
When the debounced span search query changes, `SpansScenario` MUST write the current `debouncedSearch` string into the `search_query` config key of every sibling `chat_detail` and `tool_detail` column in the workspace, and MUST clear it (set to `""`) when the search input is emptied or the selected session changes.

## Rationale
Detail columns need the active search query to drive their own highlighting and auto-expand behavior. Using `column.config` follows the existing propagation pattern (`selected_trace_id`, `selected_span_id`, `selected_tool_call_id`).

## Derived from
- [[Trace and Span Explorer]]
