---
type: test-stub
tags:
  - test/generated
---
Test file: `tests/db_module.rs`

Covers LLRs:
- [[DB open creates parent directory]] — `open_creates_missing_parent_directory_recursively`.
- [[DB open enables WAL and foreign keys]] — `open_enables_wal_and_foreign_keys` checks `PRAGMA journal_mode='wal'` and `PRAGMA foreign_keys=1`.
- [[DB open runs migrations]] — `open_runs_migrations_so_tables_are_reachable` probes migration-created tables.

Note: max-connections cap of 8 is not directly observable from a black-box test (no public accessor on `SqlitePool` for the pool size).
