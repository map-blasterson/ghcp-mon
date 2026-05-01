---
type: LLR
tags:
  - req/llr
  - domain/api
---
`GET /api/sessions` MUST return up to `limit` rows from the `sessions` table whose `last_seen_ns` is at least `since` (default 0), ordered by `last_seen_ns DESC`, where `limit` defaults to 50 and is clamped to `[1, 500]`. Each item MUST contain `conversation_id`, `first_seen_ns`, `last_seen_ns`, `latest_model`, `chat_turn_count`, `tool_call_count`, and `agent_run_count`.

## Rationale
The dashboard's session list pages by recency.

## Test context
- [[REST API Cheatsheet]]

## Derived from
- [[Dashboard REST API]]

## Test case
- [[REST API Tests]]
