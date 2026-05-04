---
type: LLR
tags:
  - req/llr
  - domain/workspace
---
No column rendered by `Workspace` SHALL be assigned fewer than `DEFAULT_MIN_COL_PX = 280` pixels of width regardless of its `width` weight or measured natural min-content width (whichever is larger), except in the degenerate case where the available container width is itself smaller than the sum of all columns' clamped minimums. The CSS `.column` class MUST also set `min-width: 280px` to enforce the same floor at the browser level.

## Rationale
Below ~280 px column scenarios (headers, selectors, tree indentation) become unusable. The JS constant and the CSS rule must agree; the JS side drives the pixel-precise grid-template calculation while the CSS side guards the minimum when the grid overflows.

## Derived from
- [[Workspace lays out columns by weight with ResizeObserver]]
