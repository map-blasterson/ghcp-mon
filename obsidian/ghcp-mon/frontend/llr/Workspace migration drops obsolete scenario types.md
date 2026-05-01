---
type: LLR
tags:
  - req/llr
  - domain/workspace
---
The `persist` `migrate` callback MUST drop any persisted column whose `scenarioType` is in the set `{"context_growth", "tool_registry", "context_inspector", "shell_io"}` before returning the state.

## Rationale
These scenario types existed in earlier versions; surviving entries would crash the `ColumnBody` switch.

## Derived from
- [[Workspace Layout]]
