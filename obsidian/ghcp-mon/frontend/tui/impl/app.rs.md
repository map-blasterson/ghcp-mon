---
type: impl
source: src/tui/app.rs
lang: rust
tags:
  - impl/generated
  - impl/rust
---
Top-level App: workspace, focus, log-overlay visibility, mouse bit; key dispatcher; drain-then-draw event loop; top-bar + workspace renderer.

## Source For
- [[Terminal Event Loop]]
- [[TUI drain pending events before draw]]
- [[TUI top bar exposes WS status dot]]
- [[TUI top bar appends column via 'a' keystroke]]
- [[TUI top bar removes focused column via 'x' keystroke]]
- [[TUI top bar shows hint string for global keys]]
- [[TUI empty workspace renders hint text]]
- [[TUI terminal MIN_COL width 24 cells]]
- [[TUI columns below MIN collapse to ellipsis label]]
- [[TUI key-dispatch precedence text-input > modal > widget > column > global]]
- [[TUI mouse capture opt-in via flag and runtime toggle]]
- [[Empty workspace shows empty state]]
- [[Default workspace seeds four columns]]
