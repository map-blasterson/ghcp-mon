---
type: HLR
tags:
  - req/hlr
  - tui
  - domain/keymap
---
All keystrokes go through one dispatcher with this precedence (highest to lowest):

1. **Text-input mode** — when an input element has focus, all printable characters + backspace/delete are consumed by the input. `Esc`, `Enter`, arrow keys are still routed normally.
2. **Modal / overlay** — debug overlay (`?`), confirm dialogs, etc. consume matching keys.
3. **Widget-local** — e.g., `SearchableTextBlock` in active state.
4. **Column scenario** — e.g., Spans tree nav.
5. **Workspace / global** — `Tab`, `q`, `Ctrl-C`, column add/remove, future `Alt-Left`/`Alt-Right` resize.

Conflicts are resolved by precedence; no key has multiple meanings at the same precedence level.

## Derived LLRs
- [[TUI key-dispatch precedence text-input > modal > widget > column > global]]
