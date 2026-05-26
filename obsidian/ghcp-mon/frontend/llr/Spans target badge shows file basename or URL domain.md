---
type: LLR
tags:
  - req/llr
  - domain/traces
---
For each span tree row, `TargetBadge` SHALL fetch the span's detail (cached under `["span", trace_id, span_id]`) and read the normalized tool-call arguments (resolution per the active vendor adapter, selected per-span by `service_name`). If the normalized file-path argument is present and is a string, the badge SHALL display its basename, splitting on `/` for POSIX paths or on `\`/`/` for Windows paths detected by the leading drive-letter pattern `/^[a-zA-Z]:[\\\/]/`. Otherwise, if the normalized target-url argument is present and is a string, the badge SHALL display the URL's hostname. Otherwise — or when the URL is malformed — the badge SHALL render nothing.

## Rationale
Seeing the target file or URL directly on the span row removes the need to open the tool detail just to know what a tool call operated on, speeding up tree scanning.

## Derived from
- [[Trace and Span Explorer]]
