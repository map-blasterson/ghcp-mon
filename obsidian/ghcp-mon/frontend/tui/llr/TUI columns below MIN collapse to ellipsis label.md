---
type: LLR
tags:
  - req/llr
  - tui
  - domain/render
---
When a column's interior width (`inner.width`) is less than 3 cells, the TUI MUST render only a dim `…` glyph at the column's top-left interior position and MUST skip the scenario body renderer for that column.

## Rationale
Prevents scenario renderers from issuing wraps or paragraphs that would crash or visually degrade at sub-3-cell widths.

## Derived from
- [[Terminal Rendering Constraints]]
