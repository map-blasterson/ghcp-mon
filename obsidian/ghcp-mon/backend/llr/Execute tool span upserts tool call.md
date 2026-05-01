---
type: LLR
tags:
  - req/llr
  - domain/normalize
---
For a span classified as `ExecuteTool`, the normalizer MUST upsert one row in `tool_calls` keyed by `span_pk`, populating `call_id` (`gen_ai.tool.call.id`), `tool_name` (`gen_ai.tool.name`), `tool_type` (`gen_ai.tool.type`), `conversation_id`, `start_unix_ns`/`end_unix_ns`/`duration_ns`, and the span's `status_code`.

## Rationale
Tool-call rows are how the dashboard counts and shows per-turn tool usage.

## Test context
- [[Normalize Pipeline Cheatsheet]]

## Derived from
- [[Span Normalization]]

## Test case
- [[Normalize Pipeline Tests]]
