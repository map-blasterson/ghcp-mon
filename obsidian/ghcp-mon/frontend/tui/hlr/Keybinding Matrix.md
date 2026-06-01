---
type: HLR
tags:
  - req/hlr
  - tui
  - domain/keymap
---
The cumulative key table registered by the TUI. Each phase appends rows; this is the Phase 0 baseline.

| Mode    | Key            | Effect                                 |
| ------- | -------------- | -------------------------------------- |
| Global  | `q`            | Quit                                   |
| Global  | `Ctrl-C`       | Quit                                   |
| Global  | `Tab`          | Focus next column                      |
| Global  | `Shift-Tab`    | Focus previous column                  |
| Global  | `?`            | Toggle log overlay                     |
| Global  | `M`            | Toggle mouse capture                   |
| Global  | `a`            | Append a column (cycle scenario type)  |
| Global  | `x`            | Remove the focused column              |
| Modal   | `?` / `Esc`    | Close log overlay                      |

## Derived LLRs
- [[TUI top bar appends column via 'a' keystroke]]
- [[TUI top bar removes focused column via 'x' keystroke]]
