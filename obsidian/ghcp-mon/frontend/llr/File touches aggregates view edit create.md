---
type: LLR
tags:
  - req/llr
  - domain/file-touches
---
`FileTouchesScenario` MUST query `api.getSessionSpanTree(session)` to obtain the full session span tree, MUST walk the tree and keep only `execute_tool` nodes whose name parses as `"execute_tool <tool_name>"` with `tool_name` in `READ_TOOLS = {"view", "read"}` or `WRITE_TOOLS = {"edit", "create", "apply_patch", "write"}`, MUST fetch each matching span's detail via `api.getSpan(trace_id, span_id)` to read `gen_ai.tool.call.arguments`, MUST extract a touched path by reading `arguments.path` (Copilot) or `arguments.filePath` (opencode) — or, for `apply_patch`, the patch headers — and MUST classify reads (`view`, `read`) as `"read"` and writes (`edit`, `create`, `apply_patch`, `write`) as `"write"`.

## Rationale
The session-span-tree endpoint surfaces tool spans as soon as they land (before the conversation_id backfill that `listSpans` requires). Copilot CLI tools (`view`/`edit`/`create`/`apply_patch`) and opencode SDK tools (`read`/`write`) are tracked together so the tree covers both producers. `apply_patch` is Copilot's multi-file edit tool; its headers contain the touched paths. opencode uses `filePath` instead of `path` for the target argument.

## Derived from
- [[File Touch Tree]]
