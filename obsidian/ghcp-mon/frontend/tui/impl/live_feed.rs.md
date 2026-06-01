---
type: impl
source: src/tui/live_feed.rs
lang: rust
tags:
  - impl/generated
  - impl/rust
---
Per-(kind,entity) ring buffer + wildcard ring, both capped at RING_MAX=500, newest-first.

## Source For
- [[TUI live feed ring buffer capped at 500 envelopes]]
- [[TUI live feed wildcard subscribers receive every envelope]]
- [[Live feed ring buffer capped at 500 envelopes]]
- [[Live feed wakes filter and wildcard subscribers]]
