---
type: LLR
tags:
  - req/llr
  - tui
  - domain/file-touches
---
Each File Touches tree row MUST be laid out left-to-right as: `2 * depth` cells of indent, a 1-cell collapse glyph (`▾` when the directory is open, `▸` when closed, blank for files), a space, the node `name`, then a right-aligned `{reads}R / {writes}W` counts column. Directory rows show their aggregate subtree counts; file rows show their per-file counts. The name MUST be truncated with a trailing `…` when the row is too narrow to fit both the name and the counts column. The focused row MUST be painted with a cyan background spanning the full row width.

## Rationale
A conventional indented filesystem view: the glyph distinguishes expandable directories from files, the right-aligned counts give an at-a-glance read/write tally per node, and the full-width cyan highlight matches the Spans / Tool Detail / Chat Detail focused-row idiom.

## Derived from
- [[File Touch Tree]]
- [[File touches builds filesystem tree with counts]]
