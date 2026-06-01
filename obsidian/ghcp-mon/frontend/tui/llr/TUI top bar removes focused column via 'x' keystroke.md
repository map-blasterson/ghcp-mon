---
type: LLR
tags:
  - req/llr
  - tui
  - domain/keymap
---
Pressing `x` at the global precedence level MUST remove the currently focused column (if any), MUST persist the workspace after removal, and MUST clamp `focused_column` to a valid index (or `None` when no columns remain).

## Rationale
Symmetric remove operation paired with `a`.

## Derived from
- [[Top Bar and Status Dot]]
- [[Keybinding Matrix]]
