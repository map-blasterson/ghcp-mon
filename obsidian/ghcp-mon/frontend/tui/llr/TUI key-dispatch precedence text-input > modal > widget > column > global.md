---
type: LLR
tags:
  - req/llr
  - tui
  - domain/keymap
---
The TUI key dispatcher MUST consult key consumers in this precedence order: (1) any focused text-input element (printable + backspace/delete); (2) any visible modal overlay; (3) any active widget-local handler (e.g., `SearchableTextBlock`); (4) the focused column's scenario; (5) the workspace/global handler. The first consumer to match consumes the event.

## Rationale
Resolves ambiguity (e.g., `q` quits globally but must not quit when typing in a search box).

## Derived from
- [[Key-Dispatch Policy]]
