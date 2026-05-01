---
type: LLR
tags:
  - req/llr
  - domain/db
---
`db::open` MUST create the parent directory of the database file (recursively, using `std::fs::create_dir_all`) when that parent does not yet exist and is not the empty path.

## Rationale
First-run UX: starting `ghcp-mon` against `./data/ghcp-mon.db` must work without a manual `mkdir`.

## Test context
- [[DB Module Cheatsheet]]

## Derived from
- [[Telemetry Persistence]]

## Test case
- [[DB Module Tests]]
