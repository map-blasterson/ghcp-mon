---
type: LLR
tags:
  - req/llr
  - domain/traces
---
For every span tree row whose `projection.tool_call.tool_name` is `edit`, `create`, or `apply_patch`, `SpansScenario` MUST render a `DiffStatBadge` that fetches the span detail (cached under `["span", trace_id, span_id]` with `staleTime: 30_000`), parses `gen_ai.tool.call.arguments`, computes added/removed line counts, and renders a red `-N` badge (`tag ib-badge-removed`) and/or a green `+M` badge (`tag ib-badge-added`) only when the corresponding count is greater than zero. Line counts SHALL be computed as follows: for `edit`, `removed = countLines(args.old_str)` and `added = countLines(args.new_str)`; for `create`, `added = countLines(args.file_text ?? args.content)` and `removed = 0`; for `apply_patch`, parse the patch text from `args.patch`, `args.input`, or the raw string-typed `args` (in that order), skip lines starting with `+++` or `---`, count lines starting with `+` as added and lines starting with `-` as removed. `countLines` MUST treat a trailing `
` as a terminator (so `"foo
"` is 1 line) and return 0 for the empty string. When both counts are zero the badge MUST render nothing.

## Rationale
Showing per-row diff size lets the user spot heavy edits, mass-creates, and noisy patches without opening the tool-detail column.

## Derived from
- [[Trace and Span Explorer]]
