---
type: LLR
tags:
  - req/llr
  - domain/db
---
`dao::insert_raw(pool, source, record_type, content_type, body)` MUST insert one row into `raw_records` with the supplied fields and return the new `id` as `i64`.

## Rationale
Single chokepoint for archive writes so every ingest path produces a uniformly-shaped raw row.

## Test context
- [[DAO Cheatsheet]]

## Derived from
- [[Telemetry Persistence]]

## Test case
- [[DAO Tests]]
