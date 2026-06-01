---
type: impl
source: src/tui/widgets/select.rs
lang: rust
tags:
  - impl/generated
  - impl/rust
---
Popover dropdown widget for the Spans session selector (`s` key) and kind filter (`k` key). Centered modal with bordered Block, scrolling list with cursor; `↑`/`↓` move cursor (handled by caller via `SelectState::move_cursor`), `Enter` selects, `Esc` closes.

## Source For
- [[Spans session selector propagates to dependent columns]]
- [[Span tree kind-filter dims non-matching rows]]
