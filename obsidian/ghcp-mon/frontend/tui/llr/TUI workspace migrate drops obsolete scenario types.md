---
type: LLR
tags:
  - req/llr
  - tui
  - domain/workspace
---
The persist `migrate` step MUST drop any persisted column whose `scenario_type` (as a string) is in the set `{"context_growth", "tool_registry", "context_inspector", "shell_io"}` before deserialization, and MUST set `schema_version` to the current `SCHEMA_VERSION` so the next save reflects current.

## Rationale
These scenario types existed in earlier webui versions; surviving entries would fail the `ScenarioType` deserializer.

## Derived from
- [[Workspace Persistence]]
- [[Workspace migration drops obsolete scenario types]]
