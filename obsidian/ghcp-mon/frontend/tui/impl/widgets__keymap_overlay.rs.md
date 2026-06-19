---
type: impl
source: src/tui/widgets/keymap_overlay.rs
lang: rust
tags:
  - impl/original
  - impl/rust
---
Modal overlay widget displaying the active keyboard bindings for the current focus context. `KeymapOverlay { entries: Vec<(String, String)> }` renders a 2-cell-inset modal with title `"keymap (? to close)"`, key column right-padded to the longest key width, yellow-bold keys, and `Wrap { trim: false }` body. Suppressed when area < 20×7.

## Source For
- [[TUI keymap overlay toggled by question mark]]
