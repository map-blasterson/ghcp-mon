---
type: LLR
tags:
  - req/llr
  - tui
  - domain/traces
---
The Spans column's search input MUST follow btop-style edit semantics:

- `/` activates the input (transitions to *text-input mode* per the Key-Dispatch Policy). Once active, text-input precedence applies — column and global keys are NOT consumed.
- `Esc` exits text-input mode back to *column-focused mode* and leaves the current query intact.
- Printable characters are inserted at the cursor verbatim.
- `Backspace` deletes one character to the left of the cursor (no-op at column 0).
- `Delete` MUST clear the entire query in one keystroke (NOT a forward-delete) and emit an empty-string change event so the search hit set collapses immediately.
- `Enter` MUST cycle to the next search hit (wrapping at the end); `Shift+Enter` MUST cycle to the previous hit (wrapping at the start). Cycling does NOT exit text-input mode.
- `←` / `→` move the cursor one char.
- `Home` / `End` jump cursor to start / end.
- Any text change emits an `on_change(text)` event to the caller; the caller applies the 300 ms debounce per [[Spans search propagates query to detail columns]] and propagates the query into every sibling `chat_detail` / `tool_detail` column's `config.search_query`.

## Rationale
Matches btop's search ergonomics — Delete-clears and Enter-cycles let the user iterate without ever leaving the input.

## Derived from
- [[Key-Dispatch Policy]]
- [[Keybinding Matrix]]
