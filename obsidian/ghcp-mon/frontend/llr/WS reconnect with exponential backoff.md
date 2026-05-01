---
type: LLR
tags:
  - req/llr
  - domain/live-events
---
After a `close` event or constructor failure, the `WsBus` MUST schedule a reconnect attempt after `min(30_000, 500 * 2^attempt)` milliseconds where `attempt` starts at 0 and increments on every scheduling, and MUST reset `attempt` to 0 on a successful `open`.

## Rationale
Exponential backoff with a 30-second ceiling keeps the dashboard recovering after backend restarts without hammering it during outages.

## Derived from
- [[Live WebSocket Subscription]]
