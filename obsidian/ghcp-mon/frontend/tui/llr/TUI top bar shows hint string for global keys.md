---
type: LLR
tags:
  - req/llr
  - tui
  - domain/workspace
---
The TUI top bar MUST render a one-line hint string after the status dot listing the supported global keys: `a:add`, `x:rm`, `Shift+←/→:move`, `Shift+Alt+←/→:resize`, `Tab:focus`, `?:logs` (treated as the help/keymap pointer), and `q:quit`. The hint string is suppressed when the top-bar area is narrower than 4 cells.

## Rationale
Discoverability for a TUI that has no menu bar.

## Derived from
- [[Top Bar and Status Dot]]
