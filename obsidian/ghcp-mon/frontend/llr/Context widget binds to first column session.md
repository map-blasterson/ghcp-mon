---
type: LLR
tags:
  - req/llr
  - domain/context-growth
---
The `ContextGrowthWidget` MUST select its session as the first column (in `useWorkspace().columns` order) whose `config.session` is set, and MUST display `"pick a session"` when no column has a session.

## Rationale
The widget is workspace-scoped, not column-scoped; "first column with a session" gives a deterministic binding.

## Derived from
- [[Context Growth Widget]]
