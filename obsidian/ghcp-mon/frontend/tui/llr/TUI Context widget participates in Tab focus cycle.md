---
type: LLR
tags:
  - req/llr
  - tui
  - domain/context-growth
---
The Context Growth Widget MUST participate in the `Tab` / `Shift-Tab` focus cycle as an extra focus slot positioned after the last column, present only while the widget is visible. Entering widget focus seeds the bar cursor to `Some(0)` when unset and publishes the initial hover. `Esc` while focused, or hiding the widget, returns focus to the columns. Widget-local keys (`←` / `→` / `Enter` / `Esc`) are dispatched in the precedence layer between modals and the focused column (`TUI key-dispatch precedence text-input > modal > widget > column > global`).

## Rationale
Folding the widget into the existing focus ring keeps a single, predictable focus model across columns and the chart.

## Derived from
- [[Keybinding Matrix]]
- [[Key-Dispatch Policy]]
