---
type: LLR
tags:
  - req/llr
  - domain/live-events
---
The live-feed module MUST keep, per distinct `kind/entity` key, a ring of the most recent envelopes ordered newest-first, and MUST cap that ring's length at `RING_MAX = 500` by truncating the tail when an arriving envelope would exceed it.

## Rationale
Bounded memory in long-running dashboard sessions; newest-first ordering matches the list scenarios.

## Derived from
- [[Live WebSocket Subscription]]
