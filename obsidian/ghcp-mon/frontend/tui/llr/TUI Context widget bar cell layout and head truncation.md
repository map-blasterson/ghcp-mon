---
type: LLR
tags:
  - req/llr
  - tui
  - domain/context-growth
---
The Context Growth Widget plot reserves a fixed left gutter of `Y_AXIS_W` cells for the y-axis labels; bars are drawn at horizontal stride `BAR_STRIDE` (cell + 1-cell gap) in the remaining width, so at most `(plot_width + 1) / 2` bars fit. When the merged turn count exceeds capacity, bars are head-truncated (`rows.iter().take(max_bars)`) so that bar index `i` always maps to `rows[i]` consistently between the draw pass and the keyboard cursor / selection handlers. The header occupies the top row, the hover/cursor underbar the bottom row, and bars fill the rows between, with each turn's stacked value mapped to cells via `cells(v) = (v / y_max) * plot_h` and clipped at the top.

## Rationale
Head truncation (not tail/scroll) keeps the bar↔row index identity trivially correct, avoiding off-by-one bugs in cursor and click routing.

## Derived from
- [[Context widget stack chart per turn]]
- [[Terminal Rendering Constraints]]
