---
type: LLR
tags:
  - req/llr
  - domain/workspace
---
`ColumnHeader` MUST render an editable title input whose `onChange` calls `updateColumn(id, { title })`, plus three action buttons that call `moveColumn(id, -1)`, `moveColumn(id, 1)`, and `removeColumn(id)`. When `children` is empty, it MUST render `"no filters"` as a muted subtitle.

## Rationale
Every scenario column needs the same chrome; per-scenario filters slot in via children.

## Derived from
- [[Workspace Layout]]
