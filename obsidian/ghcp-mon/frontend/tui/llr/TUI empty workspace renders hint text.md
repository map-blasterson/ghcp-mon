---
type: LLR
tags:
  - req/llr
  - tui
  - domain/workspace
---
When `Workspace::columns` is empty, the workspace area MUST render the literal text `"no columns. add one from the top bar."` (identical to the webui copy) inside a single bordered block, instead of the column grid.

## Rationale
Recovery hint after the user removes every column via `x`.

## Derived from
- [[Empty Workspace Hint]]
- [[Empty workspace shows empty state]]
