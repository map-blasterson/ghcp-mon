---
type: impl
source: src/tui/widgets/context_growth.rs
lang: rust
tags:
  - impl/generated
  - impl/rust
---
Context Growth Widget renderer: stacked per-turn bars (cache-read green / input blue / sub-agent lighter-blue / output orange / reasoning yellow segments), dashed token-limit line, y-axis gutter and legend header, keyboard-cursor and cross-column hover underbars, "pick a session" placeholder, collapsed single-row bar, and `plot_geometry` / `y_max` cell-mapping helpers. Head-truncates bars so bar index maps 1:1 to merged-row index. Re-exports the `merge` submodule.

## Source For
- [[Context widget stack chart per turn]]
- [[Context widget cache read green segment]]
- [[Context widget chart visual styling]]
- [[Context widget colors sub-agent input bar distinctly]]
- [[Context widget hovered chat highlights matching column]]
- [[Context widget hide and show toggle]]
- [[Context widget binds to first column session]]
- [[TUI Context widget bar cell layout and head truncation]]
- [[TUI Context widget collapsed single-row bar]]
