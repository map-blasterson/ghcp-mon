---
type: LLR
tags:
  - req/llr
  - domain/db
---
`ingest_envelope` MUST insert one `raw_records` row per envelope (with `record_type` set to the envelope type tag and `content_type='application/json'`) before normalizing the envelope, so each normalized row references its own raw record id.

## Rationale
Per-envelope raw rows give every projection a direct link back to the bytes that produced it.

## Test context
- [[Ingest Pipeline Cheatsheet]]

## Derived from
- [[Telemetry Persistence]]

## Test case
- [[Ingest Pipeline Tests]]
