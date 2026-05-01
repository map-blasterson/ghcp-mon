---
type: cheatsheet
---
Source: `src/db/dao.rs`. Crate path: `ghcp_mon::db::dao`.

## Extract

```rust
use sqlx::SqlitePool;

pub async fn insert_raw(
    pool: &SqlitePool,
    source: &str,
    record_type: &str,
    content_type: Option<&str>,
    body: &str,
) -> sqlx::Result<i64>;
```

The function inserts one row into the migration-created `raw_records` table:

```
raw_records(id INTEGER PK AUTOINCREMENT, received_at, source TEXT,
            record_type TEXT, content_type TEXT NULL, body TEXT)
```

Returned `i64` is the new `id` (RETURNING id).

Test fixture path: open a pool with `ghcp_mon::db::open(&path)` so migrations run, then call `insert_raw(&pool, ...)`.

## Suggested Test Strategy

- `#[tokio::test]` + a unique tempfile DB path (no `tempfile` dev-dep — assemble a path under `std::env::temp_dir()` per process+nanos, like `local_session::tests::tempdir_unique`).
- Acquire a pool via `ghcp_mon::db::open(&db_path).await?` so the schema exists.
- Verify behavior:
  - First call returns some `i64` ≥ 1; second call returns a strictly larger value (auto-increment).
  - After insert, `SELECT source, record_type, content_type, body FROM raw_records WHERE id = ?` returns the bound values verbatim. Pass `None` for `content_type` and assert `Option<String>` round-trips as `NULL`.
- Don't mock `SqlitePool` — use a real in-process SQLite pool. The function is a thin wrapper; mocking gives no value.
- Assert error paths by closing the pool (or pointing it at a read-only file) and confirming `sqlx::Error` variant via `assert!(result.is_err())`.
