---
type: LLR
tags:
  - req/llr
  - domain/api-client
---
The API client module MUST export `API_BASE` equal to `http://127.0.0.1:4319` and `WS_URL` equal to `ws://127.0.0.1:4319/ws/events`, and every HTTP request issued by `api.*` MUST be sent against that base URL.

## Rationale
The dashboard is bundled with and served by the backend on the same host/port; no environment-driven indirection.

## Derived from
- [[REST API Client]]
