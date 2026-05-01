---
type: LLR
tags:
  - req/llr
  - domain/normalize
---
After upserting a `sessions` row, the normalizer MUST recompute and store `chat_turn_count`, `tool_call_count`, and `agent_run_count` for that `conversation_id` from the corresponding projection tables.

## Rationale
The dashboard's session list shows up-to-date counters without computing them at read time.

## Test context
- [[Normalize Pipeline Cheatsheet]]

## Derived from
- [[Span Normalization]]

## Test case
- [[Normalize Pipeline Tests]]
