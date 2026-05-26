---
type: LLR
tags:
  - req/llr
  - domain/vendor-adapter
  - vendor/copilot
  - domain/traces
---
For Copilot `apply_patch` spans (normalized tool kind `patch` under [[Copilot tool-name mapping]]), the Copilot patch-text diff-stat parser SHALL resolve the patch text from the normalized patch-text argument (see [[Copilot tool-call argument mapping]]) and compute counts by scanning each line: lines starting with `+++` or `---` MUST be skipped (treated as headers); lines starting with `+` MUST increment the added count; lines starting with `-` MUST increment the removed count; all other lines MUST be ignored. When the patch-text argument is absent, both counts MUST be zero.

## Rationale
Surfaces per-row diff size for Copilot's multi-file patch tool in the spans column, consistent with the edit/write computation specified by the base LLR.

## Derived from
- [[Copilot tool-call shape]]
- [[Spans diff stat badges on file mutation tools]]
