---
type: LLR
tags:
  - req/llr
  - domain/api
---
`GET /api/sessions/:cid` MUST return HTTP 404 when no `sessions` row matches the conversation id; on hit it MUST return the session row plus a `span_count` field equal to the number of spans whose `attributes_json` carries `gen_ai.conversation.id == :cid`.

## Rationale
Single-session detail view needs an authoritative span count, not just the projection counters.

## Test context
- [[REST API Cheatsheet]]

## Derived from
- [[Dashboard REST API]]
- [[Uniform Error Reporting]]

## Test case
- [[REST API Tests]]
