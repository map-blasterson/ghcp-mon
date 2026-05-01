---
type: LLR
tags:
  - req/llr
  - domain/ws
---
The WebSocket handler MUST exit its loop when the client sends a `Close` frame or the receive stream returns `None`.

## Rationale
Clean teardown when the client navigates away.

## Test context
- [[WS Handler Cheatsheet]]

## Derived from
- [[Live WebSocket Event Stream]]

## Test case
- [[WS Handler Tests]]
