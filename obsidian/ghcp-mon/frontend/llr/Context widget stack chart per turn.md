---
type: LLR
tags:
  - req/llr
  - domain/context-growth
---
The chart MUST render one stacked column per chat turn (one bar per `span_pk`), with stacked sub-bars for `input`, `output`, and `reasoning` token counts, and MUST overlay a horizontal limit line at the maximum `token_limit` observed across rows. The y-axis MUST be `max(maxTokenLimit * 1.05, maxCurrent)`.

## Rationale
Stack sums to total tokens consumed; the 5% headroom over the limit keeps the limit line visible.

## Derived from
- [[Context Growth Widget]]
