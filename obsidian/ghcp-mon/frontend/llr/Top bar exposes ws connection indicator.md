---
type: LLR
tags:
  - req/llr
  - domain/workspace
---
The top bar MUST render a status dot whose `on` class is present iff `useWsStatus()` returns `true`, and whose `title` is `"connected"` when on and `"disconnected"` when off.

## Rationale
A persistent always-visible indicator of live-feed health.

## Derived from
- [[Workspace Layout]]
