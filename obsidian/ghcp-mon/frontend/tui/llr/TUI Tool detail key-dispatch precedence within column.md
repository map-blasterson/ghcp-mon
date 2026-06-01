---
type: LLR
tags:
  - req/llr
  - tui
  - domain/keymap
---
Within a focused tool-detail column, key dispatch MUST observe this precedence: (1) if the focused block is a searchable body (or an open JSON panel) whose search phase is `Active`, the key is routed to `SearchableTextBlock::handle_key` first (text-input layer); (2) otherwise `Tab`/`Shift-Tab` cycle the focusable blocks — `Tab` consumes while advancing and returns `false` (falling through to the global column-cycle, resetting `focused_block` to 0) once past the last block, `Shift-Tab` returns `false` at index 0; (3) `↑`/`↓`/`Home`/`End` scroll the column; (4) `Space` toggles the focused metadata panel or focused JSON panel (and is otherwise not consumed); (5) `/` activates search on the focused searchable/open-JSON block. Keys not matched return `false` so the global layer may handle them.

## Rationale
Layering text-input above structural navigation prevents typed query characters from triggering Tab/scroll actions, while `Tab` fall-through preserves the global column-focus cycle. `Space` is column-local (global focus uses `c`), so no conflict arises.

## Derived from
- [[Key-Dispatch Policy]]
- [[Detail columns pass span search query to TextBlocks]]
