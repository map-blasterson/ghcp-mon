---
type: LLR
tags:
  - req/llr
  - domain/file-touches
---
`FileTouchesScenario` MUST query `api.listSpans({ session, kind: "execute_tool", limit: 1000 })`, MUST keep only spans whose name parses as `"execute_tool <tool_name>"` with `tool_name` in `{"view", "edit", "create"}`, and MUST classify `view` as a `"read"` and `edit`/`create` as a `"write"`.

## Rationale
The session-scoped span list is the cheap path; tool name extraction follows the canonical span naming `execute_tool <tool_name>`.

## Derived from
- [[File Touch Tree]]
