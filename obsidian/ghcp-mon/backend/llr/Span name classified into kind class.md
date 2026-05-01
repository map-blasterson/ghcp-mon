---
type: LLR
tags:
  - req/llr
  - domain/normalize
---
`SpanKindClass::from_name(name)` MUST classify a span name as `InvokeAgent` if `name == "invoke_agent"` or `name` starts with `"invoke_agent "`, `Chat` if it starts with `"chat"`, `ExecuteTool` if it starts with `"execute_tool"`, `ExternalTool` if it starts with `"external_tool"`, and `Other` otherwise.

## Rationale
Span name is the sole input that determines which projection table is updated; classification rules must be stable and explicit.

## Test context
- [[Model Envelope Cheatsheet]]

## Derived from
- [[Span Normalization]]

## Test case
- [[Model Envelope Tests]]
