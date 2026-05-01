---
type: LLR
tags:
  - req/llr
  - domain/normalize
---
`normalize_span` MUST upsert into `spans` keyed by `(trace_id, span_id)`: on insert it creates a row with `ingestion_state='real'`; on conflict it overwrites mutable fields and forces `ingestion_state='real'`, while coalescing optional fields (`resource_json`, `scope_name`, `scope_version`) so a later partial re-delivery does not blank existing values.

## Rationale
Spans are the canonical truth and may be re-delivered; reconcile must be idempotent and never lose enrichment.

## Test context
- [[Normalize Pipeline Cheatsheet]]

## Derived from
- [[Span Normalization]]

## Test case
- [[Normalize Pipeline Tests]]
