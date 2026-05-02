---
type: LLR
tags:
  - req/llr
  - domain/traces
---
When `SpansScenario.onPickSpan` updates a column with `scenarioType === "chat_detail"` because the picked span's `kind_class` is in that column's allow-list (`["chat"]`), the patched config MUST set `selected_tool_call_id: undefined`, even if the prior config carried a value.

## Rationale
A direct chat selection is the user explicitly leaving the tool-routed flow; the auto-expand-and-arrow hint must not linger on the new chat span.

## Derived from
- [[Trace and Span Explorer]]
