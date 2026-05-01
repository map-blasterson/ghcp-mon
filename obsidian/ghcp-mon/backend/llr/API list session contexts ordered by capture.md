---
type: LLR
tags:
  - req/llr
  - domain/api
---
`GET /api/sessions/:cid/contexts` MUST return all `context_snapshots` rows whose `conversation_id` matches, ordered by `captured_ns ASC`, each item containing `ctx_pk`, `span_pk`, `captured_ns`, `token_limit`, `current_tokens`, `messages_length`, the four token counters, and `source`.

## Rationale
Time-series chart of context-window pressure across a session.

## Test context
- [[REST API Cheatsheet]]

## Derived from
- [[Dashboard REST API]]

## Test case
- [[REST API Tests]]
