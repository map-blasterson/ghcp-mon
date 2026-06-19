---
type: HLR
tags:
  - req/hlr
  - tui
  - domain/render
---
Rules shared by every scenario renderer:

- **Per-column scroll state**: every column maintains its own vertical scroll offset (`scroll_top: u16`); resize preserves the focused row when possible.
- **Focus vs input mode**: at most one column is *focused* (receives column keys); inside a focused column, at most one element is in *input mode* (receives character keys verbatim). `Esc` exits input mode; `Tab` cycles focused column.
- **Resize**: `terminal.size()` change triggers a re-layout; column widths are recomputed from weights with `MIN_COL = 24` cells (Rust analog of the web's `MIN_COL_PX = 280`); columns below MIN collapse to a truncated `…` label.
- **Wrap vs truncate**: tree rows truncate (single-line with `…` suffix); body blocks wrap (`Paragraph::wrap`).
- **Search-match offsets**: `SearchableTextBlock` (Phase 2.5) computes match offsets against the wrapped glyph stream so `scroll_into_view` works after wrap.

## Derived LLRs
- [[TUI terminal MIN_COL width 24 cells]]
- [[TUI columns below MIN collapse to ellipsis label]]
