---
type: LLR
tags:
  - req/llr
  - tui
  - domain/traces
---
The Spans column's search input MUST follow these edit semantics:

- `/` activates the input (transitions to *text-input mode* per the
  Key-Dispatch Policy). Once active, text-input precedence applies — column
  and global keys are NOT consumed.
- `Esc` exits text-input mode back to *column-focused mode* and leaves the
  current query intact.
- Printable characters are inserted at the cursor verbatim.
- `Backspace` deletes one character to the left of the cursor (does
  nothing at column 0).
- `Delete` deletes one character at the cursor (does nothing at end of
  text).
- `←` / `→` move the cursor one char.
- `Home` / `End` jump cursor to start / end.
- Any change emits an `on_change(text)` event to the caller; the caller
  applies the 300 ms debounce per
  [[Spans search propagates query to detail columns]] and propagates the
  query into every sibling `chat_detail` / `tool_detail` column's
  `config.search_query`.

## Rationale
Keeping the input vendor-free and emitting raw change events lets the
debounce + cross-column propagation live in one place (App), rather than
being duplicated per renderer.

## Derived from
- [[Key-Dispatch Policy]]
- [[Keybinding Matrix]]
