---
type: LLR
tags:
  - req/llr
  - tui
  - domain/context-growth
---
The Context Growth Widget height is stored in whole terminal rows (`context_widget_height_rows`, default 15). Any height update MUST be clamped to `3 ..= floor(0.8 * term_h)`, where `term_h` is the current terminal height in rows (the terminal analog of the web `clamp(.., 5, 80)` vh range from `Context widget drag resize 5 to 80 vh`). When the terminal height is unknown (0) the upper bound is unbounded but the floor of 3 still applies. The clamped value is persisted to the workspace TOML.

## Rationale
Clamping keeps the widget legible (≥3 rows for header + bars + underbar) and prevents it from consuming the whole screen.

## Derived from
- [[Context widget drag resize 5 to 80 vh]]
- [[Terminal Rendering Constraints]]
