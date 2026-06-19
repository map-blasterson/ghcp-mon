---
type: LLR
tags:
  - req/llr
  - tui
  - domain/context-growth
---
While the Context Growth Widget is visible, the global key layer MUST handle `Alt+↑` / `Alt+↓` to grow / shrink the widget by 1 row, and `Alt+Shift+↑` / `Alt+Shift+↓` to grow / shrink by 5 rows. Each adjustment is passed through the row clamp (`TUI Context widget height clamp in terminal rows`) and persisted. This is the keyboard analog of the web resize handle drag. When the widget is hidden, these keys are no-ops.

## Rationale
Keyboard-driven resize with a coarse (×5) modifier mirrors drag granularity without a pointer.

## Derived from
- [[Keybinding Matrix]]
- [[Context widget drag resize 5 to 80 vh]]
