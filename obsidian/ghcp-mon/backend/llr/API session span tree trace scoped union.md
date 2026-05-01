---
type: LLR
tags:
  - req/llr
  - domain/api
---
`GET /api/sessions/:cid/span-tree` MUST collect the full span set by taking every span whose attributes carry the conversation id or whose `agent_runs`/`chat_turns`/`tool_calls` row carries it as seeds, then unioning every span sharing a `trace_id` with any seed; it MUST return the resulting forest as `{conversation_id, tree}` where each node has `span_pk`, `trace_id`, `span_id`, `parent_span_id`, `name`, `kind_class`, `ingestion_state`, `start_unix_ns`, `end_unix_ns`, a `projection` block, and recursively-nested `children`. Children and roots MUST be ordered with placeholder/null-start entries first and timestamped entries newest-first.

## Rationale
A CLI session = one trace; trace-scoped union is robust against orphan placeholders that lack timestamps.

## Test context
- [[REST API Cheatsheet]]

## Derived from
- [[Dashboard REST API]]

## Test case
- [[REST API Tests]]
