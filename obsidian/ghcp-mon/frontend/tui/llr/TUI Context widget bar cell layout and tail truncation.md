---
type: LLR
tags:
  - req/llr
  - tui
  - domain/context-growth
---
The Context Growth Widget plot reserves a fixed left gutter of `Y_AXIS_W` cells for the y-axis labels; bars are drawn at horizontal stride `BAR_STRIDE` (cell + 1-cell gap) in the remaining width, so at most `(plot_width + 1) / 2` bars fit. When the merged turn count exceeds capacity, bars MUST be **tail-truncated** so the most recent `max_bars` turns remain visible (oldest turns are dropped from the left); the visible window slice is `rows[total - visible ..]`. The keyboard `bar_cursor` is expressed in **absolute** `merged.rows` coordinates: cursor and hover comparisons in the draw pass MUST add `start = total - visible` to the visible index. A cursor whose absolute index falls before `start` MUST NOT paint an underbar. The header occupies the top row, the hover/cursor underbar the bottom row, and bars fill the rows between, with each turn's stacked value mapped to cells via `cells(v) = (v / y_max) * plot_h` and clipped at the top.

## Rationale
Tail truncation matches user intent on a streaming dashboard — the newest turns are the ones the user cares about as the conversation grows. Keeping `bar_cursor` in absolute row coordinates lets the cursor remain stable when the visible window shifts on each new turn arrival, and ensures `Enter` always selects the chat span the user perceives as selected rather than a positional alias.

## Derived from
- [[Context widget stack chart per turn]]
- [[Terminal Rendering Constraints]]
