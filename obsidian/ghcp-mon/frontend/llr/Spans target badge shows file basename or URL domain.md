---
type: LLR
tags:
  - req/llr
  - domain/traces
---
For each span tree row, `TargetBadge` SHALL fetch the span's detail (cached under `["span", trace_id, span_id]`) and read its tool-call arguments. If the file-path argument is a string, the badge SHALL display its basename, splitting on `/` for POSIX paths or on `\`/`/` for Windows paths detected by the leading drive-letter pattern `/^[a-zA-Z]:[\\\/]/`. Otherwise, if the target-url argument is a string, the badge SHALL display its hostname. Otherwise — or when the URL is malformed — the badge SHALL render nothing.

## Rationale
Seeing the target file or URL directly on the span row removes the need to open the tool detail just to know what a tool call operated on, speeding up tree scanning.

## Derived from
- [[Trace and Span Explorer]]
