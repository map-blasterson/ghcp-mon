---
type: LLR
tags:
  - req/llr
  - domain/file-touches
---
`FileTouchesScenario` MUST query `api.getSessionSpanTree(session)` to obtain the full session span tree, MUST walk the tree and keep only `execute_tool` nodes whose normalized tool kind is `read`, `write`, `edit`, or `patch` (resolution per the active vendor adapter, selected per-span by `service_name`), MUST fetch each matching span's detail via `api.getSpan(trace_id, span_id)` to read `gen_ai.tool.call.arguments`, MUST extract a touched path from the normalized file-path argument (and, for `patch`-kind spans, additionally from the active vendor's patch-path extractor — multiple paths permitted per span), and MUST classify `read`-kind spans as `"read"` and `write`/`edit`/`patch`-kind spans as `"write"`.

## Rationale
The session-span-tree endpoint surfaces tool spans as soon as they land (before the conversation_id backfill that `listSpans` requires). Tracking the normalized tool-kind set (`read`/`write`/`edit`/`patch`) lets the tree cover every producer that maps into those kinds; the vendor-specific tool-name and argument resolutions live in the vendor adapter LLRs (e.g., [[Copilot tool-name mapping]], [[Copilot tool-call argument mapping]], [[opencode tool-name mapping]], [[opencode tool-call argument mapping]]). Patch-kind path extraction is delegated to the vendor (today, [[Copilot apply_patch path extraction]]).

## Derived from
- [[File Touch Tree]]
