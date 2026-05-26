---
type: LLR
tags:
  - req/llr
  - domain/traces
---
For every span tree row whose tool span has normalized tool kind in `{edit, write, patch}` (resolution per the active vendor adapter, selected per-span by `service_name`), `SpansScenario` MUST render a `DiffStatBadge` that fetches the span detail (cached under `["span", trace_id, span_id]` with `staleTime: 30_000`), parses `gen_ai.tool.call.arguments`, computes added/removed line counts from the normalized arguments, and renders a red `-N` badge (`tag ib-badge-removed`) and/or a green `+M` badge (`tag ib-badge-added`) only when the corresponding count is greater than zero. Line counts SHALL be computed as follows: for kind `edit`, `removed = countLines(<old-text argument>)` and `added = countLines(<new-text argument>)`; for kind `write`, `added = countLines(<body-text argument>)` and `removed = 0`; for kind `patch`, both counts come from the active vendor's patch-text diff-stat parser. `countLines` MUST treat a trailing `
` as a terminator (so `"foo
"` is 1 line) and return 0 for the empty string. When both counts are zero the badge MUST render nothing.

## Rationale
Showing per-row diff size lets the user spot heavy edits, mass-creates, and noisy patches without opening the tool-detail column. Vendor-specific raw argument names are resolved by the vendor adapter LLRs (e.g., [[Copilot tool-call argument mapping]], [[opencode tool-call argument mapping]]); patch-kind parsing is delegated to the vendor (today, [[Copilot apply_patch diff-stat parsing]]).

## Derived from
- [[Trace and Span Explorer]]
