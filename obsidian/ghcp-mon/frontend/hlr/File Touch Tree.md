---
type: HLR
tags:
  - req/hlr
  - domain/file-touches
---
For a selected session, the dashboard aggregates every file-touching tool call (any tool span whose normalized tool kind is `read`, `write`, `edit`, or `patch`) into a collapsible filesystem tree annotated with read/write counts, so the user can see at a glance which files the agent has touched. Patch-kind path extraction is delegated to the active vendor adapter (today, [[Copilot apply_patch path extraction]]).

## Normalized tool-call vocabulary
File-touch LLRs are specified over normalized tool kinds (`read`, `write`, `edit`, `patch`) and the normalized *file-path* argument rather than raw OTLP tool names. The active **vendor adapter** is selected per-span by `service_name`; concrete mappings live in each vendor scope (e.g., [[Copilot tool-call shape]], [[opencode tool-call shape]]).

## Derived LLRs
- [[File touches aggregates view edit create]]
- [[File touches builds filesystem tree with counts]]
- [[File touches new directories open by default]]
- [[File touches sort directories first then alphabetical]]
- [[File touches expand and collapse all controls]]
- [[File touches live invalidation on tool events]]
