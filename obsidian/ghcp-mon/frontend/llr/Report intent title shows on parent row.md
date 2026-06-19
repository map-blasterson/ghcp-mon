---
type: LLR
tags:
  - req/llr
  - domain/traces
---
For every span tree row, `SpansScenario` MUST scan the row's direct children for spans whose `projection.tool_call.tool_name === "report_intent"`, pick the latest by `start_unix_ns ?? span_pk ?? 0`, fetch its detail via `["span", trace_id, span_id]` (`staleTime: 30_000`), parse `gen_ai.tool.call.arguments` via `parseToolCallArguments`, and — when the parsed value is a non-array object whose `intent` is a non-empty string — render that text as a white-coloured title appended to the parent row (`<span style="margin-left: 6px; color: #fff">{intent}</span>`). Otherwise no title MUST be rendered.

## Rationale
The agent emits `report_intent` tool calls to announce what it is currently trying to do; surfacing that string on the parent row gives a quick narrative without expanding it.

## Derived from
- [[Trace and Span Explorer]]
