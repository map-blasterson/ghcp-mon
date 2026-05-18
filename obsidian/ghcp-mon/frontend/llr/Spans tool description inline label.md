---
type: LLR
tags:
  - req/llr
  - domain/traces
---
For every span tree row whose `projection.tool_call` is populated, `SpansScenario` MUST render a `DescriptionLabel` that fetches the span detail (cached under `["span", trace_id, span_id]` with `staleTime: 30_000`), parses `gen_ai.tool.call.arguments`, and — when `arguments.description` is a non-empty string — appends it to the row's variable-width content as an inline `<span>` with no border, no background, no `tag` chip styling, white text (`color: "#fff"`), and a 6px left margin. If `arguments.description` is missing, non-string, or empty, the label MUST render nothing.

## Rationale
The Copilot tool schema lets the model pass a `description` arg that summarises the intent of a call (e.g. for `bash`/`task` invocations). Rendering it unadorned in white makes that intent immediately scannable in the tree without taking visual weight away from the chips that classify the call.

## Derived from
- [[Trace and Span Explorer]]
