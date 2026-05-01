---
type: LLR
tags:
  - req/llr
  - domain/normalize
---
When normalizing a span whose `parent_span_id` is non-empty, the normalizer MUST issue a single idempotent `INSERT INTO spans … ON CONFLICT(trace_id, span_id) DO NOTHING RETURNING span_pk` for that `(trace_id, parent_span_id)` key, with `name=''`, `attributes_json='{}'`, and `ingestion_state='placeholder'`. The placeholder `span`/`trace` broadcast events MUST be emitted only when `RETURNING` produces a row (i.e., the row was actually inserted); when the conflict path runs (a row already exists), no events MUST fire.

## Rationale
Allows downstream span-tree views to render a child even when the parent has not yet been ingested, while keeping the operation race-free under concurrent writers and silent on no-op replays.

## Test context
- [[Normalize Pipeline Cheatsheet]]

## Derived from
- [[Span Normalization]]

## Test case
- [[Normalize Pipeline Tests]]
