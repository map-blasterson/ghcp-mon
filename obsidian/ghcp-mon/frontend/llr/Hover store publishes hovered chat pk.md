---
type: LLR
tags:
  - req/llr
  - domain/context-growth
---
`useHoverState` MUST expose a `hoveredChatPk: number | null` field with a `setHoveredChatPk(pk)` setter, implemented as a Zustand store; the value MUST NOT be persisted across reloads.

## Rationale
Transient hover state shared across the Spans column and the Context Growth widget; persistence would carry stale highlights into the next session.

## Derived from
- [[Context Growth Widget]]
