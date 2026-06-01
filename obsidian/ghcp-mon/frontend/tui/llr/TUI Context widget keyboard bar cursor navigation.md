---
type: LLR
tags:
  - req/llr
  - tui
  - domain/context-growth
---
When the Context Growth Widget holds focus, `←` / `→` MUST move a keyboard bar cursor over the merged turn bars (one step per press, clamped to `0 ..= rows.len()-1`). Each move MUST publish the cursor bar's `span_pk` into the shared `hovered_chat_pk` store so the matching Spans rows highlight in sync. The cursor is the terminal analog of pointer hover (`Context widget hovered chat highlights matching column`). `Enter` selects the cursor bar; `Esc` releases widget focus.

## Rationale
The TUI has no pointer; keyboard bar-cursor + hover publication delivers the same cross-column sync as mouse hover on the web frontend.

## Derived from
- [[Keybinding Matrix]]
- [[Context widget hovered chat highlights matching column]]
