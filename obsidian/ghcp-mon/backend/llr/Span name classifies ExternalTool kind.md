---
type: LLR
tags:
  - req/llr
  - domain/normalize
  - vendor/copilot
---
`SpanKindClass::from_name(name)` MUST additionally classify a span name as `ExternalTool` if it starts with `"external_tool"`, before falling through to `Other`.

## Rationale
GitHub Copilot emits spans with the `external_tool` prefix for tool calls made outside the agent's own tool-use loop; these need a distinct classification so downstream normalizers route them to the `external_tool_calls` table.

## Test context
- [[Model Envelope Cheatsheet]]

## Derived from
- [[Span Normalization]]

## Test case
- [[Model Envelope Tests]]
