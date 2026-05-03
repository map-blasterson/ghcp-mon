---
type: LLR
tags:
  - req/llr
  - domain/traces
---
For every span tree row whose `projection.tool_call.tool_name === "skill"`, `SpansScenario` MUST fetch the span detail via the shared `["span", trace_id, span_id]` query (with `staleTime: 30_000`), parse `gen_ai.tool.call.arguments` via `parseToolCallArguments`, and — when the parsed value is a non-array object whose `skill` property is a non-empty string — render `<span class="tag skill">{skill}</span>` on the row. When the arguments cannot be parsed or `skill` is missing/empty, the chip MUST NOT be rendered.

## Rationale
Skill invocations are a frequent and meaningful tool call; surfacing the invoked skill's name on the tree row lets users identify which skill ran without opening the tool detail panel. The dedicated `.tag.skill` style (green diagonal-stripe background) distinguishes it from the bash command chips.

## Derived from
- [[Trace and Span Explorer]]
