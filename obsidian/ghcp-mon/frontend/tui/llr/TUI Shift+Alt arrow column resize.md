---
type: LLR
tags:
  - req/llr
  - tui
  - domain/keymap
---
When a column is focused, `Shift+Alt+←` and `Shift+Alt+→` MUST adjust the focused column's layout `width` weight by `-COLUMN_RESIZE_STEP` and `+COLUMN_RESIZE_STEP` respectively (both = 0.1), clamped to `[COLUMN_WIDTH_MIN, COLUMN_WIDTH_MAX]` (= `[0.2, 5.0]`). When the change collapses against a clamp the binding MUST be a no-op. Any non-no-op resize MUST persist the workspace.

## Rationale
Lets the user widen the spans column relative to detail columns without removing or re-adding them.

## Derived from
- [[Keybinding Matrix]]
