---
type: LLR
tags:
  - req/llr
  - tui
  - domain/input-breakdown
---
Inside a focused Chat Detail column, `Tab`/`Shift-Tab` MUST fall through to the global column-focus cycle; there is no within-column block focus cycle. The tree row cursor remains the primary column-local focus surface for `↑`/`↓`/`←`/`→`/`Space`, and within an Active per-block search the text-input precedence layer consumes printable keys, `Backspace`, and `Esc` per [[TUI key-dispatch precedence text-input > modal > widget > column > global]].

## Rationale
Keeping `Tab`/`Shift-Tab` reserved for global column-focus cycling makes column movement predictable and prevents users from tabbing through sub-blocks inside Chat Detail.

## Derived from
- [[Chat detail long primitives click to expand]]
- [[Detail columns pass span search query to TextBlocks]]
- [[TUI key-dispatch precedence text-input > modal > widget > column > global]]
