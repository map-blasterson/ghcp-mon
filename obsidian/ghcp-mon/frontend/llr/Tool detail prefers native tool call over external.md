---
type: LLR
tags:
  - req/llr
  - domain/tool-detail
---
When `SpanDetail.projection.tool_call` is populated, `ToolDetailScenario` MUST render `ToolDetailBody` and MUST NOT render `ExternalToolDetailBody`, regardless of whether `projection.external_tool_call` is also populated.

## Rationale
A span can carry both projections (paired tool-call rows for the same span). The native `tool_call` row is richer (carries `tool_type`, `status_code`, and drives the specialized arg renderers), so it takes precedence.

## Derived from
- [[Tool Call Inspector]]
