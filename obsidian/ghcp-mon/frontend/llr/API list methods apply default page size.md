---
type: LLR
tags:
  - req/llr
  - domain/api-client
---
When the caller does not supply `limit`, `api.listSessions` and `api.listTraces` MUST default it to `50`, `api.listSpans` and `api.listRaw` MUST default it to `100`. The default MUST be sent as the `limit` query parameter on the underlying request.

## Rationale
The backend clamps limits on its side; the client picks page sizes that balance perceived freshness and bandwidth per scenario.

## Derived from
- [[REST API Client]]
