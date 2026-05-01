---
type: LLR
tags:
  - req/llr
  - domain/db
---
`db::open` MUST configure the SQLite connection with `create_if_missing=true`, `journal_mode=WAL`, `synchronous=Normal`, and `foreign_keys=true`, and MUST cap the pool at 8 connections.

## Rationale
WAL + Normal sync gives concurrent reads alongside ingest; foreign keys enforce projection integrity; the bounded pool prevents fd exhaustion under load.

## Test context
- [[DB Module Cheatsheet]]

## Derived from
- [[Telemetry Persistence]]

## Test case
- [[DB Module Tests]]
