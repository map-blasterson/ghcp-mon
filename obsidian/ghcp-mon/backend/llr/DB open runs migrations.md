---
type: LLR
tags:
  - req/llr
  - domain/db
---
`db::open` MUST run all migrations under the `./migrations` directory (embedded via `sqlx::migrate!`) against the pool before returning.

## Rationale
Schema is owned by the binary; the database file is brought up to the current schema on every startup.

## Test context
- [[DB Module Cheatsheet]]

## Derived from
- [[Telemetry Persistence]]

## Test case
- [[DB Module Tests]]
