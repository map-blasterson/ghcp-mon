---
type: LLR
tags:
  - req/llr
  - domain/text-block
---
When `TextBlock`'s active search phase is driven by a non-empty `externalQuery` prop, the component SHALL NOT exit the active phase in response to interactive exit gestures — specifically, the Escape key, column-body `mouseleave`, and right-click context-menu dismiss. The search lifecycle SHALL be owned exclusively by the `externalQuery` prop: the active phase ends only when `externalQuery` transitions to empty or `undefined`.

## Rationale
Externally-driven search (e.g., from the Spans column's search box propagated to ChatDetail/ToolDetail via `config.search_query`) must not be cancelled by accidental user gestures inside the highlighted block. The external owner controls the query lifecycle; the block is a passive consumer.

## Derived from
- [[Searchable Text Block]]
