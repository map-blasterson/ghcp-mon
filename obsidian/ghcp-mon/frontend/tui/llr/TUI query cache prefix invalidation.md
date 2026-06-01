---
type: LLR
tags:
  - req/llr
  - tui
  - domain/cache
---
`QueryCache::invalidate(prefix)` MUST match every key whose initial segments equal `prefix`, bump each matched entry's `latest_gen` by 1, and mark each matched entry's cached value stale by adjusting its `fetched_at` so `is_stale()` returns true.

## Rationale
Used by the WS coalescer to mark dependent caches stale on each `(kind, entity)` envelope.

## Derived from
- [[Async Query Cache]]
