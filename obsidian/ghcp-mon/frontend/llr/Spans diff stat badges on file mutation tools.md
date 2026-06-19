---
type: LLR
tags:
  - req/llr
  - domain/traces
---
For every span tree row whose tool kind is in `{edit, write, patch}`, `SpansScenario` MUST render a `DiffStatBadge` that fetches the span detail (cached under `["span", trace_id, span_id]` with `staleTime: 30_000`), parses `gen_ai.tool.call.arguments`, computes added/removed line counts, and renders a red `-N` badge (class `tag ib-badge-removed`) and/or a green `+M` badge (class `tag ib-badge-added`) only when the corresponding count is greater than zero. Counts SHALL be computed as: for `edit`, `removed = countLines(<old-text>)` and `added = countLines(<new-text>)`; for `write`, `added = countLines(<body-text>)` and `removed = 0`; for `patch`, counts come from the patch-text diff-stat parser. `countLines` MUST treat a trailing `
` as a terminator (so `"foo
"` is 1 line) and return 0 for the empty string. When both counts are zero the badge MUST render nothing.

## Rationale
Showing per-row diff size lets the user spot heavy edits, mass-creates, and noisy patches without opening the tool-detail column.

## Derived from
- [[Trace and Span Explorer]]
