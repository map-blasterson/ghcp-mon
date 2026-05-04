---
type: LLR
tags:
  - req/llr
  - domain/workspace
---
The `Workspace` component SHALL measure each column's natural minimum width by cloning the column DOM element off-screen with `width: min-content`, reading its `getBoundingClientRect().width`, removing the clone, and clamping the result to at least `DEFAULT_MIN_COL_PX` (280). These measured minimums SHALL be kept in sync with actual content via a `ResizeObserver` and `MutationObserver` (attributes, childList, characterData, subtree) on every column element, scheduled through `requestAnimationFrame` to coalesce rapid changes.

## Rationale
Columns contain variable-width content (headers, selectors, search inputs, tree indentation) that can push their intrinsic minimum above the static default. Measuring the live DOM ensures the layout engine never assigns a column less space than its content requires, preventing overflow and text truncation. Observing both resize and mutation events catches content changes (e.g., a newly-rendered tool chip) that would not trigger a resize alone.

## Derived from
- [[Workspace Layout]]
