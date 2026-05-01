---
type: test-stub
tags:
  - test/generated
---
Test file: `tests/dao.rs`

Covers LLR:
- [[DAO insert_raw returns row id]] — `insert_raw_returns_row_id_and_persists_fields` checks (a) returned `i64` ≥ 1, (b) auto-increment, (c) all bound fields round-trip in `raw_records`, (d) `None` content_type round-trips as NULL.
