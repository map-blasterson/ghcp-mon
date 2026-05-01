---
type: LLR
tags:
  - req/llr
  - domain/api
---
`GET /api/spans` MUST return up to `limit` spans (default 100, clamped to `[1, 1000]`), ordered by `start_unix_ns DESC`, optionally filtered by `since` (minimum `start_unix_ns`), `session` (matching either the span's own `gen_ai.conversation.id` attribute or any of its `agent_runs`/`chat_turns`/`tool_calls` rows), and `kind`. The `kind` filter MUST be applied in SQL via a `CASE` expression over `name` that mirrors `SpanKindClass::from_name` (`'invoke_agent'` for `name = 'invoke_agent'` or names prefixed `'invoke_agent '`; `'chat'` for `name LIKE 'chat%'`; `'execute_tool'` for `name LIKE 'execute_tool%'`; `'external_tool'` for `name LIKE 'external_tool%'`; else `'other'`) — applied in the `WHERE` clause before `LIMIT` so the page is never undercounted.

## Rationale
The dashboard's span search needs server-side filters (session, since, kind) that don't truncate results, and the kind classifier must stay consistent with normalization (`SpanKindClass::from_name`).

## Test context
- [[REST API Cheatsheet]]

## Derived from
- [[Dashboard REST API]]

## Test case
- [[REST API Tests]]
