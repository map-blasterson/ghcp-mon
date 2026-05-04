---
type: LLR
tags:
  - req/llr
  - domain/input-breakdown
---
When `column.config.search_query` is non-empty, `ChatDetailScenario` MUST compute the set of tree node IDs whose text content contains the query (case-insensitive), then add every ancestor node ID of each matching node to the `expanded` set without removing any user-expanded nodes. When the query is cleared, the previously search-expanded nodes SHOULD be removed from the expanded set, restoring the user's prior expand state.

## Rationale
Auto-expanding only the ancestor path keeps the tree navigable and avoids overwhelming the user. This follows the same non-destructive expand pattern used by the tool-call hint auto-expand.

## Derived from
- [[Chat detail]]
