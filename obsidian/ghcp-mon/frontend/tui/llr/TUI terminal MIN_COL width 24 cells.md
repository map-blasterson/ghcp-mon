---
type: LLR
tags:
  - req/llr
  - tui
  - domain/render
---
The TUI workspace layout MUST enforce a minimum column width of 24 cells (the Rust analog of the web's `MIN_COL_PX = 280`). When the terminal is too narrow for every weighted column to meet this minimum, columns below the minimum width MUST render only the truncated ellipsis `…` label inside their border.

## Rationale
Below 24 cells a scenario renderer cannot show meaningful content; collapsing to `…` preserves the column slot for keyboard navigation.

## Derived from
- [[Terminal Rendering Constraints]]
