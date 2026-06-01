---
type: LLR
tags:
  - req/llr
  - tui
  - domain/cache
---
Every `QueryCache::begin_fetch` MUST bump the per-key `latest_gen` counter by 1. `QueryCache::put(rec)` MUST discard `rec` whose `generation < latest_gen`, leaving the cached value untouched.

## Rationale
An invalidation that fires while a fetch is in flight MUST not have its result clobbered by the older fetch's late completion.

## Derived from
- [[Async Query Cache]]
