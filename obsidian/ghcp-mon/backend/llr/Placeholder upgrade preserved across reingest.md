---
type: LLR
tags:
  - req/llr
  - domain/normalize
---
When the real span for a `(trace_id, span_id)` arrives after a placeholder row was inserted for that key, the upsert MUST flip `ingestion_state` from `'placeholder'` to `'real'`, and the broadcast event for that span MUST set `action="upgrade"` (vs `"insert"` when no prior row existed).

## Rationale
Lets clients distinguish first-ingest from a placeholder being filled in.

## Test context
- [[Normalize Pipeline Cheatsheet]]

## Derived from
- [[Span Normalization]]

## Test case
- [[Normalize Pipeline Tests]]
