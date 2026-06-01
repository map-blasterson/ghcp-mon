---
type: impl
source: src/tui/event.rs
lang: rust
tags:
  - impl/generated
  - impl/rust
---
Event channel + coalescer + crossterm reader + animation tick: bounded mpsc(256), per-frame WS-tick coalescer, blocking poll task.

## Source For
- [[Terminal Event Loop]]
- [[TUI bounded event channel with coalesced WS ticks]]
- [[TUI coalescer accumulates dirty prefixes per frame]]
