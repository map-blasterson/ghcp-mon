---
type: LLR
tags:
  - req/llr
  - domain/workspace
---
The `Workspace` component MUST observe its container's pixel width via `ResizeObserver`, MUST distribute that width across columns proportionally to each column's `width` weight (after subtracting `4 px` per inter-column resizer), MUST clamp every column to at least `MIN_COL_PX = 120` and redistribute the deficit to the unpinned columns, MUST round to integer pixels, and MUST absorb the rounding remainder into the last unpinned column so the pixel sum equals the available width.

## Rationale
Pixel-precise layout avoids fr-unit sub-pixel drift across browser zoom levels and split-screen resize.

## Derived from
- [[Workspace Layout]]
