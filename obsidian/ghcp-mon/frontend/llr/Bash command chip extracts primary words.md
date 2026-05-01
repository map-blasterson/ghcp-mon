---
type: LLR
tags:
  - req/llr
  - domain/traces
---
For every span tree row whose `projection.tool_call.tool_name === "bash"`, `SpansScenario` MUST fetch the span detail via `["span", trace_id, span_id]`, parse `gen_ai.tool.call.arguments.command`, split it on whitespace-bounded `&&`, `||`, or `|` separators, take the first non-`KEY=value` token of each segment, basename it, truncate at 24 chars (with `…`), and render the result as up to 6 hash-coloured chips with a `…` overflow chip when there are more.

## Rationale
Surfaces what shell pipelines copilot is running without opening the tool detail panel.

## Derived from
- [[Trace and Span Explorer]]
