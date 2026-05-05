---
type: LLR
tags:
  - req/llr
  - domain/traces
---
When the user selects a span of kind `invoke_agent`, `SpansScenario` SHALL locate the most recent chat descendant of that agent node (by `sortKey`: `end_unix_ns` → `start_unix_ns` → `span_pk`) and advance any `chat_detail` column's `selected_span_id` to that chat span, so the user immediately sees the sub-agent's conversation.

## Rationale
An invoke_agent span itself has no chat content; routing to its latest chat descendant gives the user immediate context without requiring a manual second click into the sub-tree.

## Derived from
- [[Trace and Span Explorer]]
