---
type: LLR
tags:
  - req/llr
  - domain/ws
---
On accepting a WebSocket upgrade at `/ws/events`, the server MUST send the JSON text frame `{"kind":"hello","entity":"control","payload":{"server":"ghcp-mon"}}` before forwarding any broadcast events; if that send fails the connection MUST be closed.

## Rationale
Hello frame lets clients confirm they are connected to a `ghcp-mon` server before subscribing UI handlers.

## Test context
- [[WS Handler Cheatsheet]]

## Derived from
- [[Live WebSocket Event Stream]]

## Test case
- [[WS Handler Tests]]
