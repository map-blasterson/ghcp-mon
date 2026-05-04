---
type: impl
source: web/src/scenarios/Spans.tsx
lang: rust
tags:
  - impl/generated
  - impl/rust
---
Adds a search text input to the `SpansScenario` column config bar with 300ms debounce. When active, queries `GET /api/search` and passes matching span_ids to `SpanTreeView`/`SpanTreeNode` which apply `search-hit` and `dim` CSS classes. Clearing search or deselecting session restores normal view.

## Satisfies
- [[Spans searchbox queries server on input]]
- [[Spans search results highlight matching nodes]]
