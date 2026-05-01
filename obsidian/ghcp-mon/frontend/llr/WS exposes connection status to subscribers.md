---
type: LLR
tags:
  - req/llr
  - domain/live-events
---
`wsBus.onStatus(listener)` MUST immediately invoke `listener` with the current `connected` boolean, MUST then invoke `listener(true)` on every `open` and `listener(false)` on every `close`, and MUST return an unsubscribe function that removes the listener.

## Rationale
New subscribers need the current state synchronously so they can render the status dot without a flash of "disconnected".

## Derived from
- [[Live WebSocket Subscription]]
