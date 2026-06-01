---
type: LLR
tags:
  - req/llr
  - tui
  - domain/live-events
---
Every envelope ingested into `LiveFeed` MUST also be appended to a shared wildcard ring buffer (capped at `RING_MAX`), so a workspace-wide subscriber can read every envelope by reading the wildcard ring.

## Rationale
Matches the web `useLiveFeed` wildcard `"*"` subscription channel.

## Derived from
- [[Async Query Cache]]
- [[Live feed wakes filter and wildcard subscribers]]
