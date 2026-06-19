---
type: LLR
tags:
  - req/llr
  - domain/file-touches
---
`FileTouchesScenario` MUST query `api.getSessionSpanTree(session)` for the full session span tree, MUST walk it and keep only `execute_tool` nodes whose tool kind is `read`, `write`, `edit`, or `patch`, MUST fetch each matching span's detail via `api.getSpan(trace_id, span_id)` to read `gen_ai.tool.call.arguments`, MUST extract a touched path from the file-path argument (for `patch`, multiple paths may be extracted from the patch text), and MUST classify `read`-kind spans as `"read"` and `write`/`edit`/`patch`-kind spans as `"write"`.

## Rationale
The session-span-tree endpoint surfaces tool spans as soon as they land, before the conversation_id backfill `listSpans` requires. The file-touch tree covers every producer whose tools map into the four kinds.

## Derived from
- [[File Touch Tree]]
