---
type: LLR
tags:
  - req/llr
  - domain/ws
  - domain/normalize
---
On creating a placeholder span row (i.e., the idempotent insert actually inserted a new row), the normalizer MUST broadcast a `kind="span"`, `entity="placeholder"` event with `action="insert"`, `trace_id`, `span_id`, and `span_pk`, plus a `kind="trace"`, `entity="trace"` event with `action="placeholder"` and `ingestion_state="placeholder"`. When the placeholder insert is a no-op (the row already exists), no events MUST fire.

## Rationale
The UI shows an in-flight placeholder for a not-yet-ingested parent.

## Test context
- [[Normalize Pipeline Cheatsheet]]

## Derived from
- [[Live WebSocket Event Stream]]
- [[Span Normalization]]

## Test case
- [[Normalize Pipeline Tests]]
