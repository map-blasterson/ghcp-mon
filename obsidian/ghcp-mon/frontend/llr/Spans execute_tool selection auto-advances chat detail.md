---
type: LLR
tags:
  - req/llr
  - domain/traces
---
When the user picks a span whose `kind_class === "execute_tool"` in the spans column and a session span tree is loaded, `SpansScenario` MUST: (1) for every column whose `scenarioType === "chat_detail"` for which `execute_tool` is NOT in the kind allow-list, locate the next chat-kind sibling of the picked tool span (the first chat-class span among the picked span's `parent_span_id`-siblings whose `(end_unix_ns ?? start_unix_ns ?? span_pk ?? 0, span_pk)` sort key strictly exceeds the picked span's), and update that column's config with `selected_trace_id`, `selected_span_id` set to that chat sibling, and `selected_tool_call_id` set to `picked.projection.tool_call?.call_id`; (2) when no such chat sibling exists in the loaded tree, leave the column's selection unchanged.

## Rationale
Execute-tool selections should auto-route the chat-detail column to the chat turn that consumed the tool's response, with the tool call id available so chat detail can target the matching `tool_call_response` part.

## Derived from
- [[Trace and Span Explorer]]
- [[Span selection routes by kind class allow list]]
