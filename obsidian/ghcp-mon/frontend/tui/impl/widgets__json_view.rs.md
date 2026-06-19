---
type: impl
source: src/tui/widgets/json_view.rs
lang: rust
tags:
  - impl/generated
  - impl/rust
---
`JsonView` widget + `JsonView::pretty(value) -> String`: 2-space-indented JSON pretty-print that never panics, with collapsed/open summary glyphs (`▸ json…` / `▾ json…`). Terminal port of the web `JsonView` with `collapsed` default.

## Source For
- [[JsonView pretty prints with optional collapse]]
- [[TUI JsonView collapsed default closed]]
