---
type: LLR
tags:
  - req/llr
  - domain/live-events
---
On a `WebSocket` `error` event, `WsBus` MUST call `close()` on the underlying socket so the standard `close` handler fires the reconnect path.

## Rationale
Centralizes reconnect to one code path regardless of whether failure surfaces as `error` or `close`.

## Derived from
- [[Live WebSocket Subscription]]
