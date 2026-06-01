---
type: impl
source: src/tui/cache.rs
lang: rust
tags:
  - impl/generated
  - impl/rust
---
Async query cache: in-flight dedupe via broadcast, generation-based stale suppression, prefix invalidation, stale-while-revalidate, LRU eviction at 1024 for span keys, and the WS→prefix invalidation table.

## Source For
- [[TUI query cache in-flight dedupe]]
- [[TUI query cache generation-based stale suppression]]
- [[TUI query cache prefix invalidation]]
- [[TUI query cache stale-while-revalidate]]
- [[TUI query cache LRU evicts span keys at 1024]]
- [[TUI WS to query-key invalidation table]]
