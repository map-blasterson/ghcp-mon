---
type: LLR
tags:
  - req/llr
  - domain/live-events
---
The `wsBus` MUST be exported as a singleton, and `wsBus.start()` MUST open a `WebSocket` to `WS_URL` only on its first call; subsequent calls while a socket exists MUST be no-ops.

## Rationale
A single shared connection multiplexes events for every subscriber and avoids stampedes on hot reload.

## Derived from
- [[Live WebSocket Subscription]]
