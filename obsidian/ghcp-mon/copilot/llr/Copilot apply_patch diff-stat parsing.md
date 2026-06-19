---
type: LLR
tags:
  - req/llr
  - domain/vendor-adapter
  - vendor/copilot
  - domain/traces
---
For Copilot `apply_patch` spans, the patch-text diff-stat parser SHALL scan the patch-text argument line by line: lines starting with `+++` or `---` are skipped; `+`-prefixed lines increment the added count; `-`-prefixed lines increment the removed count; all other lines are ignored. When patch-text is absent, both counts MUST be zero.

## Rationale
Surfaces per-row diff size for Copilot's multi-file patch tool in the spans column.

## Derived from
- [[Copilot tool-call shape]]
- [[Spans diff stat badges on file mutation tools]]
