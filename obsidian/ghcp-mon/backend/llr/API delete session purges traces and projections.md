---
type: LLR
tags:
  - req/llr
  - domain/api
---
`DELETE /api/sessions/:cid` MUST return HTTP 404 when no `sessions` row matches `:cid`; on hit it MUST, within a single transaction, delete every `spans` row whose `trace_id` is reachable from the conversation (any span carrying `gen_ai.conversation.id == :cid`, or whose `span_pk` is referenced by an `agent_runs`/`chat_turns`/`tool_calls` row tagged with that conversation), then delete remaining `context_snapshots`, `hook_invocations`, `skill_invocations`, `external_tool_calls`, `tool_calls`, `chat_turns`, `agent_runs`, and the `sessions` row tagged with `:cid`, then publish a `derived`/`session` event with `action: "delete"` and `conversation_id: :cid`, and return `{deleted: true, conversation_id, trace_count}`.

## Rationale
Cleanup of a session must remove every domain row tied to it — spans, projections, and aggregates — so the dashboard's session list reflects truth and disk usage shrinks.

## Test context
- [[REST API Cheatsheet]]

## Derived from
- [[Dashboard REST API]]
- [[Telemetry Persistence]]
- [[Live WebSocket Event Stream]]
- [[Uniform Error Reporting]]

## See also
- [[Session upsert emits derived session event]] — index of legal `action` values for `derived`/`session` events.

## Test case
- [[REST API Tests]]
