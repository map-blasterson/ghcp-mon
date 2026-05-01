---
type: LLR
tags:
  - req/llr
  - domain/live-events
---
On every WebSocket `message` event, `WsBus` MUST `JSON.parse` the payload, treat the result as a `WsEnvelope`, and invoke every registered listener (registered via `wsBus.on`) with that envelope.

## Rationale
Listeners are the integration point for the per-`(kind, entity)` ring-buffer fan-out in `state/live.ts`.

## Derived from
- [[Live WebSocket Subscription]]
- [[WS forwards broadcast events to client]]
