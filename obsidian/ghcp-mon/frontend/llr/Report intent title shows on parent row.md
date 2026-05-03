---
type: LLR
tags:
  - req/llr
  - domain/traces
---
For every span tree row, `SpansScenario` MUST scan the row's direct children for spans whose `projection.tool_call.tool_name === "report_intent"`, pick the one with the largest `start_unix_ns ?? span_pk ?? 0` (the latest by start time), fetch its detail via the shared `["span", trace_id, span_id]` query (`staleTime: 30_000`), parse `gen_ai.tool.call.arguments` via `parseToolCallArguments`, and — when the parsed value is a non-array object whose `intent` property is a non-empty string — render that `intent` text as a white-coloured title appended to the parent row (`<span style="margin-left: 6px; color: #fff">{intent}</span>`). When no `report_intent` child exists or its `intent` argument is missing/non-string/empty, no title MUST be rendered.

## Rationale
The Copilot agent emits `report_intent` tool calls to announce what it is currently trying to do; surfacing that string on the parent (typically `invoke_agent` or `chat`) row gives a quick narrative of the agent's plan without expanding the row.

## Derived from
- [[Trace and Span Explorer]]
