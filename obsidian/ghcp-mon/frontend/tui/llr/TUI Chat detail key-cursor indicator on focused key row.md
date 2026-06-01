---
type: LLR
tags:
  - req/llr
  - tui
  - domain/input-breakdown
---
In place of the web `KeyCursorIcon` (which renders `[+]` / `[-]` floating at the pointer position), the TUI MUST render a `(+)` (collapsed) or `(-)` (expanded) static indicator inline at the **right** side of the focused primitive key row. The indicator is painted only when the focus row corresponds to a long primitive (string length > 200 or contains `
`); short primitives never get the indicator. The indicator MUST be styled `Color::Yellow` BOLD so it is visible without competing with the row's main label colour. This is a deliberate terminal substitute — the mouse-following icon has no terminal-cell analog per the §"Mouse capture is opt-in" rule.

## Rationale
Hover-tracking is not available without mouse capture (which is opt-in). A static row-aligned indicator gives the same affordance with zero per-frame cursor work and degrades gracefully when mouse mode is off.

## Derived from
- [[Chat detail key cursor icon follows pointer]]
- [[Chat detail long primitives click to expand]]
