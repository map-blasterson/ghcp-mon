# db — Memory Bank (backend)

## Purpose
SQLite persistence layer for the backend: opens (and bootstraps) the database file, configures the connection pool for safe concurrent ingest+read, runs embedded schema migrations on startup, and exposes thin DAO functions that are the single chokepoint for writes. Owns durability and projection integrity for telemetry, but holds no business logic — callers (`ingest/`, `normalize/`, `api/`) compose DAOs into their workflows.

## Key behaviors
- Configures the SQLite pool with `create_if_missing=true`, `journal_mode=WAL`, `synchronous=Normal`, `foreign_keys=true`, and caps the pool at 8 connections — *DB open enables WAL and foreign keys*.
- Recursively creates the parent directory of the database file (via `std::fs::create_dir_all`) when it does not exist and is not the empty path, so first-run against e.g. `./data/ghcp-mon.db` works without a manual `mkdir` — *DB open creates parent directory*.
- Runs all migrations under `./migrations` (embedded via `sqlx::migrate!`) against the pool before `open` returns, bringing the database file up to the current schema on every startup — *DB open runs migrations*.
- `dao::insert_raw(pool, source, record_type, content_type, body)` inserts one row into `raw_records` with the supplied fields and returns the new `id` as `i64` — *DAO insert_raw returns row id*.

## Public surface
- `db::open` — creates the parent directory if needed, constructs the SQLite pool with WAL / `synchronous=Normal` / `foreign_keys=ON` / `create_if_missing=true` / max 8 connections, and runs embedded migrations before returning the pool.
- `db::Pool` (sqlx SQLite pool) — handed to `AppState` and shared by `ingest/`, `normalize/`, and `api/`.
- `dao::insert_raw(pool, source, record_type, content_type, body) -> i64` — sole archive-write entry point; produces uniformly-shaped rows in `raw_records`.
- Additional DAO functions used by `normalize/` (projection upserts) and `api/` (read paths) are implementation details; the vault LLRs in scope only specify `insert_raw`. Read source for the full DAO surface.

## Invariants & constraints
- WAL mode, `synchronous=Normal`, `foreign_keys=ON`, `create_if_missing=true`, pool cap = 8 connections (LLR: *DB open enables WAL and foreign keys*).
- Parent directory of the DB file is created recursively when missing and non-empty — filesystem side effect of `db::open` (LLR: *DB open creates parent directory*).
- Migrations under `./migrations` run on every `open`, before the pool is returned to callers (LLR: *DB open runs migrations*). Schema is owned by the binary.
- DAO contract: `insert_raw` writes exactly one `raw_records` row with the supplied `source`, `record_type`, `content_type`, `body` and returns the new row id as `i64` — single chokepoint for archive writes (LLR: *DAO insert_raw returns row id*).
- Foreign keys exist to enforce projection integrity from normalized rows back to their source raw record (per HLR traceability requirement).

## Dependencies
- Schema definitions live in the repo's `migrations/` folder (filesystem, not vault). The vault does not model individual tables/columns as LLRs but references the directory by name in *DB open runs migrations*; concrete table/column shapes (including `raw_records`) are defined there.
- Consumed by `ingest/` — calls `dao::insert_raw` to archive each OTLP envelope as its own raw record.
- Consumed by `normalize/` — performs projection upserts from raw rows into normalized telemetry tables.
- Consumed by `api/` — read paths over normalized (and possibly raw) tables.
- Provides `Pool` to `AppState`, which is the shared handle threaded through the rest of the backend.

## Notes
- The `migrations/` folder is repo-only and embedded into the binary via `sqlx::migrate!`. Concrete schema details (table names, columns, indices, FK targets) live there, *not* in the vault — when reasoning about column-level behavior, read `migrations/` directly.
- Per HLR *Telemetry Persistence*, the layer's job is durability, idempotent reconcile across re-deliveries, and traceability from derived rows back to their source raw record. The four in-scope LLRs cover bootstrap + the raw-write chokepoint; the HLR also derives sibling LLRs (*Raw request body persisted verbatim per OTLP request*, *Each envelope persisted as own raw record*) which are owned by `ingest/`, not `db/`.
- Pool cap of 8 is deliberate (fd-exhaustion guard under load); WAL + `synchronous=Normal` is the chosen point on the durability/throughput curve to allow concurrent reads alongside ingest writes.

## Where to read for detail
- HLR: `backend/hlr/Telemetry Persistence.md`
- LLRs:
  - `backend/llr/DB open enables WAL and foreign keys.md`
  - `backend/llr/DB open creates parent directory.md`
  - `backend/llr/DB open runs migrations.md`
  - `backend/llr/DAO insert_raw returns row id.md`
- Source: `src/db/mod.rs`, `src/db/dao.rs`
- Out-of-vault reference: repo-root `migrations/` (embedded via `sqlx::migrate!`)
