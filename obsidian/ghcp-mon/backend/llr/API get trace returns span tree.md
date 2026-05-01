---
type: LLR
tags:
  - req/llr
  - domain/api
---
`GET /api/traces/:trace_id` MUST return HTTP 404 when no spans belong to the trace; on hit it MUST return `{trace_id, conversation_id, tree}` where `tree` is the same span-tree shape as `/api/sessions/:cid/span-tree`.

## Rationale
Reuses the span-tree builder so trace and session detail views share rendering logic.

## Test context
- [[REST API Cheatsheet]]

## Derived from
- [[Dashboard REST API]]
- [[Uniform Error Reporting]]

## Test case
- [[REST API Tests]]
