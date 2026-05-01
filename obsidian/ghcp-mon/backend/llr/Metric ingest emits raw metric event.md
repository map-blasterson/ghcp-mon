---
type: LLR
tags:
  - req/llr
  - domain/ws
  - domain/normalize
---
After persisting a metric envelope, the normalizer MUST broadcast a `kind="metric"`, `entity="metric"` event whose payload contains `name` and `points` (the count of data points in the envelope).

## Rationale
Lightweight notification — full metric data is fetched on demand, so the event only carries summary fields.

## Test context
- [[Normalize Pipeline Cheatsheet]]

## Derived from
- [[Live WebSocket Event Stream]]
- [[Span Normalization]]

## Test case
- [[Normalize Pipeline Tests]]
