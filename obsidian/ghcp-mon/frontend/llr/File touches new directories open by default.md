---
type: LLR
tags:
  - req/llr
  - domain/file-touches
---
When a directory path appears in the touched tree for the first time, `FileTouchesScenario` MUST add it to the open-directory set; directories already known MUST keep their current open/closed state across live updates.

## Rationale
Newly-discovered files default to visible while preserving the user's explicit collapses.

## Derived from
- [[File Touch Tree]]
