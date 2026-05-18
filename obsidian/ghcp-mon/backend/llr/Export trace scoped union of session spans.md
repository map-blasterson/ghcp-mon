---
type: LLR
tags:
  - req/llr
  - domain/export
---
`export::export_session` MUST select every span whose `ingestion_state = 'real'` and whose `trace_id` belongs to the union of (a) trace_ids of spans whose attributes carry `gen_ai.conversation.id = conv_id`, and (b) trace_ids of spans referenced by any `agent_runs`, `chat_turns`, `tool_calls`, or `external_tool_calls` projection row tagged with `conversation_id = conv_id`. Spans with `ingestion_state = 'placeholder'` MUST NOT be exported.

## Rationale
A CLI session corresponds to one or more traces; including every sibling span in those traces preserves parent/child chains so replay reconstructs the full hierarchy. Placeholder rows are skeletal and would not round-trip cleanly.

## Derived from
- [[Session Export]]
