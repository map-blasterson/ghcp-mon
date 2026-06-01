---
type: LLR
tags:
  - req/llr
  - tui
  - domain/input-breakdown
---
The Chat Detail tool-call hint arrow MUST be rendered in a dedicated 1-cell gutter at column position 0 of the tree area (left of the focus glyph), painting `▶` only at the row of the message returned by `auto_expand_for_tool_call`. All other tree rows MUST paint a blank space in this cell — the gutter occupancy is constant regardless of whether the arrow is visible. At most one arrow MUST be visible at any time (the single matching `tool_call_response` message); when `selected_tool_call_id` is unset or no matching message exists, no arrow is painted.

## Rationale
A dedicated gutter keeps the arrow visually separated from row content and tree indent; constant-width occupancy prevents reflow when the arrow appears or disappears. Limiting to one arrow matches the web's single `[data-ib-id]` target.

## Derived from
- [[Chat detail tool-call hint auto-expand and arrow]]
