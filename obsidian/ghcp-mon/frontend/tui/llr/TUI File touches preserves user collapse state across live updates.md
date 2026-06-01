---
type: LLR
tags:
  - req/llr
  - tui
  - domain/file-touches
---
`FileTouchesState` MUST track a `known_dirs` set alongside `open_dirs`. On each render, for every directory path present in the freshly-built tree: if the path is not in `known_dirs`, the column MUST add it to both `known_dirs` and `open_dirs` (auto-open on first appearance); if the path is already in `known_dirs`, the column MUST NOT alter its membership in `open_dirs` (preserving the user's explicit collapse/expand across live ingest ticks).

## Rationale
The render runs once per frame and rebuilds the tree from the cache every tick, so the auto-open rule must be idempotent: gating it on `known_dirs` ensures a directory is force-opened exactly once — the first time it is seen — while subsequent ticks leave the user's manual state untouched.

## Derived from
- [[File Touch Tree]]
- [[File touches new directories open by default]]
