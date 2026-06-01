---
type: HLR
tags:
  - req/hlr
  - tui
  - domain/cache
---
The TUI hosts an async in-memory query cache (TanStack-Query analog) with these clauses:

1. **In-flight dedupe** — a fetch in progress for key K causes a duplicate `get(K)` to await the same future.
2. **Generation-based stale suppression** — every fetch carries a monotonic per-key generation; on completion, results from older generations are discarded.
3. **Prefix invalidation** — `invalidate(prefix: &[&str])` matches any key whose initial segments equal the prefix; matched entries' generations are bumped and their cached values marked stale.
4. **Stale-while-revalidate** — `peek(key)` returns the cached value immediately if present and dispatches a refetch when stale.
5. **LRU eviction** — capped at 1024 entries for keys whose first segment is `"span"`; other key classes are uncapped (they're inherently small).
6. **WS table** — incoming envelopes invalidate cache prefixes per `ws_invalidation_prefixes(kind, entity)`.
7. **Shared keys** — `["session-span-tree", cid]` stores the complete tree; visual reveal filtering is a local derivation. Chat-detail DELTA's "prior chat span" walk MUST use the complete cached tree.

## Derived LLRs
- [[TUI query cache in-flight dedupe]]
- [[TUI query cache generation-based stale suppression]]
- [[TUI query cache prefix invalidation]]
- [[TUI query cache stale-while-revalidate]]
- [[TUI query cache LRU evicts span keys at 1024]]
- [[TUI WS to query-key invalidation table]]
