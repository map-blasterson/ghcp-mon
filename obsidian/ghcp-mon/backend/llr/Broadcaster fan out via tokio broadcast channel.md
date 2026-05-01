---
type: LLR
tags:
  - req/llr
  - domain/ws
---
`Broadcaster::new(cap)` MUST construct a `tokio::sync::broadcast` channel with the supplied capacity, and `Broadcaster::send(msg)` MUST emit the message to every current subscriber while ignoring send errors when there are no subscribers.

## Rationale
Unbounded fan-out from a single producer to every connected dashboard tab.

## Test context
- [[Broadcaster Cheatsheet]]

## Derived from
- [[Live WebSocket Event Stream]]

## Test case
- [[Broadcaster Tests]]
