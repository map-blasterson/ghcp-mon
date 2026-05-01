---
type: LLR
tags:
  - req/llr
  - domain/normalize
---
After upserting a span and any projection row, the normalizer MUST walk the span's ancestors (up to depth 64, joining `spans` on `trace_id` + `parent_span_id`) and back-fill `parent_agent_run_pk`/`parent_span_pk` on the row's `agent_runs`, and `agent_run_pk`/`chat_turn_pk`/`conversation_id` on its `chat_turns`/`tool_calls`/`external_tool_calls`/`hook_invocations`/`skill_invocations` projections, using `COALESCE` so existing values are not overwritten.

## Rationale
Projections store flat parent pointers for cheap querying; ancestor walk derives them from the recursive parent chain whenever any new span lands.

## Test context
- [[Normalize Pipeline Cheatsheet]]

## Derived from
- [[Span Normalization]]

## Test case
- [[Normalize Pipeline Tests]]
