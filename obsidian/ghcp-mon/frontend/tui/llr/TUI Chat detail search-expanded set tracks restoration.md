---
type: LLR
tags:
  - req/llr
  - tui
  - domain/input-breakdown
---
When `column.config.search_query` transitions from empty to non-empty, the Chat Detail scenario MUST snapshot the current `expanded` set into `search_expanded_snapshot` BEFORE applying the auto-expand walk's union. When the query transitions back to empty, the snapshot MUST be restored verbatim into `expanded`, dropping all search-driven expansions; user-driven expansions made *during* the search remain (because they were folded into `user_expanded` independently). The snapshot lifecycle is per-column.

## Rationale
This matches the web LLR's "previously search-expanded nodes SHOULD be removed when the query is cleared, restoring the user's prior expand state." The snapshot is the simplest correct way to recover the pre-search state without tracking which expansions came from the search walk vs the user.

## Derived from
- [[ChatDetail auto-expands tree to span search matches]]
