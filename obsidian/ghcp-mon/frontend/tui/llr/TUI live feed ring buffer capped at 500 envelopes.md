---
type: LLR
tags:
  - req/llr
  - tui
  - domain/live-events
---
`LiveFeed::ingest` MUST append the envelope to a per-`(WsKind, WsEntity)` ring buffer (newest-first), truncating the tail so the ring's length never exceeds `RING_MAX = 500`. The wildcard ring MUST follow the same cap.

## Rationale
Bounded memory in long-running attach sessions; newest-first ordering matches the list scenarios.

## Derived from
- [[Async Query Cache]]
- [[Live feed ring buffer capped at 500 envelopes]]
