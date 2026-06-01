---
type: LLR
tags:
  - req/llr
  - tui
  - domain/cache
---
When `QueryCache::put` records a value whose key starts with `"span"`, the cache MUST move that key to the front of an LRU and, after insertion, MUST evict the least-recently-used `"span"` keys until the LRU has at most `SPAN_LRU_CAP = 1024` entries. Eviction MUST remove the evicted key's cache entry entirely. Non-span keys are not subject to the cap.

## Rationale
Bounded growth for the unbounded set of per-span detail queries; every other key class is small (< 100 entries).

## Derived from
- [[Async Query Cache]]
