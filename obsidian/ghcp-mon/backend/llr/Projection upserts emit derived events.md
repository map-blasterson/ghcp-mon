---
type: LLR
tags:
  - req/llr
  - domain/ws
  - domain/normalize
---
After upserting an `agent_runs`, `chat_turns`, or `tool_calls` row, the normalizer MUST broadcast a `kind="derived"` event whose `entity` is `"agent_run"`, `"chat_turn"`, or `"tool_call"` respectively, carrying the projection's identifying ids (e.g., `agent_run_pk`/`turn_pk`/`tool_call_pk`), `span_pk`, `conversation_id`, and projection-specific fields (`agent_name`, `interaction_id`/`turn_id`, `tool_name`/`call_id`/`status_code`).

## Rationale
Per-projection events let the UI update specific tables without re-querying.

## Test context
- [[Normalize Pipeline Cheatsheet]]

## Derived from
- [[Live WebSocket Event Stream]]
- [[Span Normalization]]

## Test case
- [[Normalize Pipeline Tests]]
