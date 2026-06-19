---
type: LLR
tags:
  - req/llr
  - tui
  - domain/input-breakdown
---
The Chat Detail column body MUST split vertically into three regions: row 0 = a 1-row header strip (chat title chip + total bytes + `mode: [DELTA|FULL]` chip + the `m=toggle` hint), row 1 = a 1-row summary bar painted by [[TUI Chat detail summary bar paints via Buffer cell_mut]], rows 2..end = the scrollable tree. Each tree row uses a 2-cell prefix: cell 0 is the arrow gutter ([[TUI Chat detail tool-call arrow gutter]]), cell 1 is the focus glyph (`▸`/blank for unfocused, `▾`/blank for collapse-state — separated to keep the focus state distinct from the expand glyph). Each nesting level adds 2 cells of indent under its parent.

## Rationale
The header + bar are fixed; the tree gets the remainder. Splitting the prefix into arrow vs focus gutter keeps the visual cues independent: the arrow only ever appears on the tool-call target row, while the focus glyph follows the cursor.

## Derived from
- [[Chat detail]]
- [[Chat detail summary bar proportional to visible segments]]
