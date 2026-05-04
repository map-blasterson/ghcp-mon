---
type: impl
source: src/api/mod.rs
lang: rust
tags:
  - impl/generated
  - impl/rust
---
Adds the `search_spans` handler implementing `GET /api/search` with session-scoped full-text search across span name, attributes, tool_calls, agent_runs, and span_events. Also adds the `SearchQuery` parameter struct and 400 validation for missing `session`/`q`.

## Satisfies
- [[API search spans by text within session]]
- [[API search requires session and query parameters]]
