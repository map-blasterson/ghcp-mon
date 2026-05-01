---
type: LLR
tags:
  - req/llr
  - domain/workspace
---
The shared `QueryClient` constructed in `main.tsx` MUST set `defaultOptions.queries` to `{ refetchOnWindowFocus: false, retry: 1, staleTime: 5_000 }`.

## Rationale
Live invalidation is driven by the WS feed, so focus refetch is unnecessary; a 5-second stale window dampens redundant fetches across columns sharing a query key.

## Derived from
- [[Workspace Layout]]
