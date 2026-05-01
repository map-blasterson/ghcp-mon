---
type: LLR
tags:
  - req/llr
  - domain/ws
---
On receiving a WebSocket `Ping` frame from the client, the handler MUST respond with a `Pong` frame carrying the same payload bytes.

## Rationale
Standard WebSocket keepalive; some clients (browsers, intermediaries) require explicit Pong replies.

## Test context
- [[WS Handler Cheatsheet]]

## Derived from
- [[Live WebSocket Event Stream]]

## Test case
- [[WS Handler Tests]]
