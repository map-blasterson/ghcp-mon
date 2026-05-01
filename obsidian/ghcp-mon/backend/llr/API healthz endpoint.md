---
type: LLR
tags:
  - req/llr
  - domain/api
---
`GET /api/healthz` MUST return HTTP 200 with the JSON body `{"ok": true}`.

## Rationale
Liveness probe used by orchestrators and the dashboard.

## Test context
- [[REST API Cheatsheet]]

## Derived from
- [[Dashboard REST API]]

## Test case
- [[REST API Tests]]
