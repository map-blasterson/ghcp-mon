---
type: impl
source: src/tui/cache.rs
lang: rust
tags:
  - impl/generated
  - impl/rust
---
Async query cache: in-flight dedupe via broadcast, generation-based stale suppression, prefix invalidation, stale-while-revalidate, LRU eviction at 1024 for span keys, and the WS→prefix invalidation table. Phase 1 adds `cache_get(cache, key, stale_after, fetcher)` — a convenience async wrapper composing peek + begin_fetch + put + finish_fetch + in-flight dedupe so scenarios get a single-call fetch API.

## Source For
- [[TUI query cache in-flight dedupe]]
- [[TUI query cache generation-based stale suppression]]
- [[TUI query cache prefix invalidation]]
- [[TUI query cache stale-while-revalidate]]
- [[TUI query cache LRU evicts span keys at 1024]]
- [[TUI WS to query-key invalidation table]]
- [[Async Query Cache]]
- [[Default TanStack Query options no refetch on focus]]
