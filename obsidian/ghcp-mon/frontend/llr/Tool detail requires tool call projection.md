---
type: LLR
tags:
  - req/llr
  - domain/tool-detail
---
`ToolDetailScenario` MUST render the empty state `"selected span is not a tool call"` whenever the resolved `SpanDetail.projection` has neither a `tool_call` nor an `external_tool_call` populated, regardless of the span's `kind_class`.

## Rationale
Drives correctness when a non-tool span (e.g. an `other` placeholder) is force-selected: the user needs an explicit "wrong selection" hint. External-origin tool spans (MCP) carry an `external_tool_call` projection and are routed to a dedicated body instead of this empty state.

## Derived from
- [[Tool Call Inspector]]
