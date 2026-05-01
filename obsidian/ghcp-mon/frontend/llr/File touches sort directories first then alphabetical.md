---
type: LLR
tags:
  - req/llr
  - domain/file-touches
---
Children of any tree node MUST be sorted with directories (`children.size > 0`) before files, and within each group alphabetically by `name` via `localeCompare`.

## Rationale
Stable, conventional filesystem ordering.

## Derived from
- [[File Touch Tree]]
