---
type: LLR
tags:
  - req/llr
  - domain/live-events
---
When a WebSocket envelope with key `kind/entity` arrives, `useLiveFeed` MUST notify every subscriber registered under that exact key AND every subscriber registered under the wildcard key `"*"`, by invoking each registered callback once.

## Rationale
Components subscribe to specific event classes for targeted invalidation; a wildcard channel is reserved for whole-workspace listeners.

## Derived from
- [[Live WebSocket Subscription]]
