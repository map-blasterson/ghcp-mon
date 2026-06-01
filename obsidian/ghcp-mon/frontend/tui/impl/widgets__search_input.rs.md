---
type: impl
source: src/tui/widgets/search_input.rs
lang: rust
tags:
  - impl/generated
  - impl/rust
---
Single-line text input for the Spans header searchbox: `←`/`→` cursor moves with UTF-8 char-boundary safety, `Home`/`End`, `Backspace`, `Delete`, printable char entry. `take_changed()` lets the caller detect buffer mutations for the 300 ms debounce + propagation to detail columns. `Esc` is intentionally NOT consumed here — the App decides whether to exit text-input mode.

## Source For
- [[TUI Spans search input edit semantics]]
- [[Spans searchbox queries server on input]]
