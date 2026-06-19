---
type: LLR
tags:
  - req/llr
  - tui
  - domain/cache
---
`QueryCache::peek(key)` MUST return the cached value immediately when present, and MUST report `will_refetch = true` when the entry is missing, stale, or has no in-flight fetch covering it.

## Rationale
Scenarios re-render with stale data while the refresh is in flight, matching TanStack-Query semantics.

## Derived from
- [[Async Query Cache]]
