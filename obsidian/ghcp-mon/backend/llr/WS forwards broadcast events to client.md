---
type: LLR
tags:
  - req/llr
  - domain/ws
---
After the hello, the WebSocket handler MUST forward every `EventMsg` received from the broadcaster to the client as a JSON text frame; on send failure the loop MUST exit and the socket MUST be closed.

## Rationale
Per-client send errors must not stall the broadcast bus or leak tasks.

## Test context
- [[WS Handler Cheatsheet]]

## Derived from
- [[Live WebSocket Event Stream]]

## Test case
- [[WS Handler Tests]]
