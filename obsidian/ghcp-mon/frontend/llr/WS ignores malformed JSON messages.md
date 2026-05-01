---
type: LLR
tags:
  - req/llr
  - domain/live-events
---
If `JSON.parse` throws on an incoming WebSocket `message`, `WsBus` MUST swallow the error and MUST NOT invoke any listener for that message.

## Rationale
Malformed frames must not crash the dashboard or take down the connection.

## Derived from
- [[Live WebSocket Subscription]]
