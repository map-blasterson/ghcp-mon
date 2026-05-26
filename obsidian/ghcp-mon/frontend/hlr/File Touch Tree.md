---
type: HLR
tags:
  - req/hlr
  - domain/file-touches
---
For a selected session, the dashboard aggregates every file-touching tool call (Copilot's `view`/`edit`/`create`/`apply_patch` and opencode's `read`/`write`) into a collapsible filesystem tree annotated with read/write counts, so the user can see at a glance which files the agent has touched.

## Derived LLRs
- [[File touches aggregates view edit create]]
- [[File touches builds filesystem tree with counts]]
- [[File touches new directories open by default]]
- [[File touches sort directories first then alphabetical]]
- [[File touches expand and collapse all controls]]
- [[File touches live invalidation on tool events]]
- [[File touches extracts paths from apply_patch headers]]
