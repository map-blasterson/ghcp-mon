---
type: LLR
tags:
  - req/llr
  - domain/tool-detail
---
`ExternalToolDetailBody` MUST render its "args / result" section using `GenericArgs` for every external tool, regardless of `ext.tool_name`. It MUST NOT dispatch to the specialized `EditArgs`, `ViewArgs`, `ReadAgentArgs`, or `TaskArgs` renderers.

## Rationale
The specialized renderers are tuned to specific built-in CLI tool names. External-origin tool calls (MCP servers, etc.) have arbitrary, server-defined names that don't map to those shapes, so the generic key-aware renderer is the only safe default.

## Derived from
- [[Tool Call Inspector]]
