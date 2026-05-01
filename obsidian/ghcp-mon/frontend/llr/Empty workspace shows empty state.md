---
type: LLR
tags:
  - req/llr
  - domain/workspace
---
When `useWorkspace().columns` is empty, the `Workspace` component MUST render the empty-state message `"no columns. add one from the top bar."` instead of the grid layout.

## Rationale
Removing all columns is reachable via the per-column ✕ button; the user needs a recovery hint.

## Derived from
- [[Workspace Layout]]
