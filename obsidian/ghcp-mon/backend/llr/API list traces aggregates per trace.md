---
type: LLR
tags:
  - req/llr
  - domain/api
---
`GET /api/traces` MUST return one row per `trace_id` containing `first_seen_ns`, `last_seen_ns`, `span_count`, `placeholder_count`, `kind_counts` (a fixed map of the five kind classes to their counts), a `root` span object (the span with NULL parent or whose parent is not in the trace, ordered earliest first), and the trace's `conversation_id` (any span carrying `gen_ai.conversation.id`). Results MUST be filtered by `since` and limited as in the other list endpoints.

## Rationale
The traces list is the dashboard's primary live feed, replacing per-session lists for at-a-glance monitoring.

## Test context
- [[REST API Cheatsheet]]

## Derived from
- [[Dashboard REST API]]

## Test case
- [[REST API Tests]]
