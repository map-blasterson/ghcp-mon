---
type: LLR
tags:
  - req/llr
  - domain/traces
---
For each span tree row, `TargetBadge` SHALL fetch the span's detail (cached under `["span", trace_id, span_id]`) and parse `gen_ai.tool.call.arguments`. If the parsed arguments contain a `path` string property, the badge SHALL display the file's basename (splitting on `/` for Unix paths or `\`/`/` for Windows paths detected by a leading drive letter pattern). If no `path` is present but a `url` string property exists, the badge SHALL display the URL's hostname. If neither property is present or the URL is malformed, the badge SHALL render nothing.

## Rationale
Seeing the target file or URL directly on the span row removes the need to open the tool detail just to know what a tool call operated on, speeding up tree scanning.

## Derived from
- [[Trace and Span Explorer]]
