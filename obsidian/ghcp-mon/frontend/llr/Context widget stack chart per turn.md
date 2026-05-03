---
type: LLR
tags:
  - req/llr
  - domain/context-growth
---
The chart MUST render one stacked column per chat turn (one bar per `span_pk`), with stacked sub-bars for `input`, `output`, and `reasoning` token counts, and MUST overlay a dotted yellow horizontal limit line at the maximum `token_limit` observed across rows (rows without a `token_limit` do not contribute to that maximum). When at least one snapshot carries a `token_limit`, the y-axis MUST be `max(maxTokenLimit * 1.10, maxCurrent)`; when no snapshot carries a `token_limit`, the y-axis MUST be `maxCurrent`.

## Rationale
Anchoring the y-axis to `1.10 * maxTokenLimit` keeps the dotted yellow limit line visible with ~10% headroom above it, while extending to `maxCurrent` ensures bars that exceed the limit are still shown in full. Falling back to just the tallest bar when no `token_limit` is known avoids guessing a default and keeps every bar visible.

## Derived from
- [[Context Growth Widget]]
