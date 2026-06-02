---
type: LLR
tags:
  - req/llr
  - tui
  - domain/chat-detail
---
The Chat Detail column MUST render a single unified focus marker (yellow `▶` glyph) in the tree's prefix cell at `state.focus_row`, replacing the previous split between a "tool-follow arrow" gutter and an arrow-key cursor highlight. The same `state.focus_row` MUST drive both user arrow-key navigation AND external `selected_tool_call_id` snap: on selection change the renderer MUST snap `focus_row` to the matching tool-call message row exactly once, after which user arrow keys retain control until the next external selection.

## Rationale
Eliminates the dual-cursor ambiguity where the visible "tool follow" arrow and the keyboard cursor pointed at different rows.

## Derived from
- [[TUI Chat detail key-cursor indicator on focused key row]]
- [[Chat detail tool-call hint auto-expand and arrow]]
