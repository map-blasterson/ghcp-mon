---
type: LLR
tags:
  - req/llr
  - tui
  - domain/cache
---
`QueryCache::begin_fetch(key)` MUST return an existing in-flight ticket when a fetch is already in progress for `key` (same generation, `already_in_flight = true`, a broadcast receiver awaiting the same value); otherwise it MUST mint a new generation and a fresh broadcast sender.

## Rationale
Prevents N concurrent renders for the same key from issuing N HTTP requests.

## Derived from
- [[Async Query Cache]]
