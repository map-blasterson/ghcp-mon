---
type: LLR
tags:
  - req/llr
  - tui
  - domain/text-block
---
When `truncatable` is true and `open` is false, `SearchableTextBlock` MUST render only the wrapped rows in `[scroll_top, scroll_top + truncate_rows)` (capped at the available body height) and, when hidden rows remain below the window (`scroll_top + plot_h < total_rows`), overwrite the **last visible row's final cell** with a `…` glyph styled `Color::DarkGray` + `Modifier::DIM`. When `open` is true the block renders the full body and never paints the ellipsis. Match highlighting still applies to truncated rows, and `match_count` MUST include matches located on rows beyond the truncated range (they are counted even though they are not painted). The parent owns the `open` flag via per-node state; the block renders no built-in chevron/expand affordance.

## Rationale
This is the controlled "click-to-expand" mode from the web `TextBlock`, re-expressed for cells: a fixed-height preview with an ellipsis tail, fully driven by the parent's `open` prop. Counting hidden matches keeps the header counter ("N of M") honest even while the body is collapsed.

## Derived from
- [[TextBlock truncatable controlled by open prop]]
- [[Terminal Rendering Constraints]]
