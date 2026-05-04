---
type: LLR
tags:
  - req/llr
  - domain/file-touches
---
`FileTouchesScenario` MUST query `api.getSessionSpanTree(session)` to obtain the full session span tree, MUST walk the tree and keep only `execute_tool` nodes whose name parses as `"execute_tool <tool_name>"` with `tool_name` in `{"view", "edit", "create", "apply_patch"}`, MUST fetch each matching span's detail via `api.getSpan(trace_id, span_id)` to read `gen_ai.tool.call.arguments`, and MUST classify `view` as a `"read"` and `edit`/`create`/`apply_patch` as a `"write"`.

## Rationale
The session-span-tree endpoint surfaces tool spans as soon as they land (before the conversation_id backfill that `listSpans` requires). `apply_patch` is a write-tool used by Copilot for multi-file edits; its headers (`*** Add File:`, `*** Update File:`, `*** Delete File:`, `Move to:`) contain the touched paths. Tool name extraction follows the canonical span naming `execute_tool <tool_name>`.

## Derived from
- [[File Touch Tree]]
