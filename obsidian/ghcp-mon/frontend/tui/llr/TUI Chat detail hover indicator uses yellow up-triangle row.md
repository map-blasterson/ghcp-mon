---
type: LLR
tags:
  - req/llr
  - tui
  - domain/chat-detail
---
The Chat Detail column MUST render a dedicated hover-indicator row immediately below the summary bar that paints yellow `▲` characters spanning the cell range of the summary-bar segment (or the contiguous descendants of an ancestor) matching the focused tree row's node id. When no segment matches (or no row is focused), the row MUST be left blank. Matching MUST treat the hovered id as either exact-equal to a segment id or a `"{hovered}/"` prefix, since node ids are slash-delimited paths.

## Rationale
Replaces the previous "brighten the hovered bar segment" interaction; an explicit pointer row is readable on any palette and survives DELTA's shading.

## Derived from
- [[TUI Chat detail summary bar paints via Buffer cell_mut]]
