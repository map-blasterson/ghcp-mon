---
type: LLR
tags:
  - req/llr
  - domain/ws
  - domain/normalize
---
After upserting a span, the normalizer MUST broadcast two `EventMsg` records: one with `kind="span"`, `entity="span"` carrying `action`, `trace_id`, `span_id`, `parent_span_id`, `name`, `kind_class`, `ingestion_state="real"`, and `span_pk`; and one with `kind="trace"`, `entity="trace"` carrying `action`, `trace_id`, `span_id`, `ingestion_state="real"`, and `upgraded` (true iff a placeholder was upgraded).

## Rationale
Live span and trace lists in the UI both consume these events.

## Test context
- [[Normalize Pipeline Cheatsheet]]

## Derived from
- [[Live WebSocket Event Stream]]
- [[Span Normalization]]

## Test case
- [[Normalize Pipeline Tests]]
