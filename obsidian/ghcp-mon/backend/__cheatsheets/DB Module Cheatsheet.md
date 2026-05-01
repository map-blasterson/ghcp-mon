---
type: cheatsheet
---
Source: `src/db/mod.rs`. Crate path: `ghcp_mon::db`.

Migrations live in `./migrations/` (relative to crate root). They are embedded by `sqlx::migrate!` at compile time.

## Extract

```rust
use sqlx::SqlitePool;
use std::path::Path;

pub mod dao;

pub async fn open(db_path: &Path) -> anyhow::Result<sqlx::SqlitePool>;
```

Behavior surface (no bodies):
- `open` is `async`, returns `anyhow::Result<SqlitePool>`.
- It accepts `&std::path::Path` to a SQLite database file (the file may or may not exist).
- The pool is constructed from `sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions}` using a URL of the form `sqlite://<path>?mode=rwc` and configured with `create_if_missing(true)`, `journal_mode(Wal)`, `synchronous(Normal)`, `foreign_keys(true)`, `max_connections(8)`.
- Migrations are run via `sqlx::migrate!("./migrations").run(&pool).await?` before returning.
- Parent directory of `db_path` is created with `std::fs::create_dir_all` when non-empty.

Returned type: `sqlx::SqlitePool` (alias of `sqlx::Pool<sqlx::Sqlite>`). `anyhow::Error` wraps any underlying `sqlx::Error` / `std::io::Error` / `MigrateError`.

## Suggested Test Strategy

- Use `#[tokio::test]`. Pick a unique temp path per test (e.g. `std::env::temp_dir().join(format!("ghcp-mon-test-{pid}-{nanos}.db"))`) and clean up after. The `local_session.rs` tests already use this pattern — mirror it. The crate doesn't depend on `tempfile`; build paths manually or add it to dev-deps if desired.
- For "creates parent directory": pass a path whose parent does **not** exist (e.g. `tempdir.join("nested/deep/db.sqlite")`) and after `open` returns, assert that `parent.exists()` is true.
- For "WAL + foreign keys": after `open`, run probes via `sqlx::query_scalar` against the returned pool:
  - `PRAGMA journal_mode;` should yield `"wal"`.
  - `PRAGMA foreign_keys;` should yield `1` (i64).
- For "runs migrations": after `open`, assert one of the migration-created tables is queryable (e.g. `SELECT COUNT(*) FROM raw_records`, `... FROM spans`, `... FROM sessions` — all defined by migrations under `./migrations/`). Don't hard-code migration version numbers; assert table reachability instead.
- `sqlx::migrate!("./migrations")` is resolved relative to `CARGO_MANIFEST_DIR` at compile time, so tests work regardless of cwd at runtime.
- For error paths, point the function at an unwritable parent (e.g. `/dev/null/x`) and assert `result.is_err()`.
