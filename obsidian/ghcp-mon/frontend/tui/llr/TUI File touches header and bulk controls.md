---
type: LLR
tags:
  - req/llr
  - tui
  - domain/file-touches
---
The File Touches column body MUST reserve row 0 for a 1-row header strip containing, left-to-right: a session marker chip (`⊞ files`), the aggregate `(total_R R / total_W W)` counts summed across the tree roots, and — right-aligned when space permits — the `[+]` / `[-]` bulk-control indicators. The bulk controls MUST render in a DIM/disabled style when no directories are present, and in an enabled (cyan/bold) style otherwise. Rows 1..end hold the scrollable tree.

## Rationale
The header is the terminal analog of the web column header's totals badge and expand/collapse buttons. Rendering the controls DIM communicates the same "disabled when empty" affordance the web `disabled` attribute provides, without using a separate sentinel.

## Derived from
- [[File Touch Tree]]
- [[File touches expand and collapse all controls]]
