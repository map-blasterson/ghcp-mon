---
type: LLR
tags:
  - req/llr
  - domain/input-breakdown
---
In `DELTA` mode, `buildToolDefsNode` MUST diff `current` vs `prior` tool-definition arrays by `name` (with deep-equality multiset matching for unnamed entries): same name + equal content counts as unchanged; same name with differing content emits both a `REMOVED` (prior) and an `ADDED` (current) child; names only in current are `ADDED`; names only in prior are `REMOVED`. When all tools are unchanged, emit a single `tool_def_unchanged` node (no children); otherwise emit a `tool_def_root` node whose children are the REMOVED nodes followed by the ADDED nodes, each carrying its respective `badge`.

## Rationale
Name-keyed diffing matches how tool definitions evolve across turns (re-emitted whole, occasionally renamed); the `badge` carries through to the visual chips.

## Derived from
- [[Chat detail DELTA diffs against prior chat span]]
