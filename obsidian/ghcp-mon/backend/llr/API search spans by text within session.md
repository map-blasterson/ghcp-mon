---
type: LLR
tags:
  - req/llr
  - domain/api
---
`GET /api/search?q=<query>&session=<cid>&limit=<n>` MUST return up to `limit` spans (default 50, clamped to `[1, 200]`), scoped to the given `session` conversation_id, whose `name`, `tool_calls.tool_name`, `agent_runs.agent_name`, `attributes_json` values, or associated `span_events` name or attributes contain the `q` text (case-insensitive substring match). Each result MUST include `span_pk`, `trace_id`, `span_id`, `name`, `kind_class`, `start_unix_ns`, `end_unix_ns`, `ingestion_state`, projection, and a `matches` array of `{field, fragment}` objects identifying which field(s) matched.

## Rationale
The dashboard needs server-side full-text search so the user can locate spans by tool name, attribute value, or event content without scrolling through the tree. Session scoping keeps results focused on the active investigation.

## Derived from
- [[Dashboard REST API]]
