---
type: cheatsheet
---
Source: `src/ingest/replay.rs`. Crate path: `ghcp_mon::ingest::replay`.

Depends on: `crate::server::AppState` (see [[Server Router Cheatsheet]]) and `crate::ingest::ingest_jsonl_file` (see [[Ingest Pipeline Cheatsheet]]).

## Extract

```rust
use axum::{extract::State, response::IntoResponse, Json};
use serde::Deserialize;
use crate::server::AppState;
use crate::error::AppResult;

#[derive(Deserialize)]
pub struct ReplayReq { pub path: String }

pub async fn replay(
    State(state): State<AppState>,
    Json(req): Json<ReplayReq>,
) -> AppResult<impl IntoResponse>;
```

The handler:
- Parses `req.path` into `std::path::PathBuf`.
- Calls `crate::ingest::ingest_jsonl_file(&state, &path, "replay").await?` and gets back a `usize` count.
- Returns `Json(json!({"path": req.path, "ingested": count}))` on success.
- Failure paths return `AppError` (rendered with `{"error": "..."}`); see [[AppError Cheatsheet]].

`AppError::BadRequest` arises when the JSONL file can't be opened (bubbles up from `ingest_jsonl_file`).

## Suggested Test Strategy

- Mount under a small `Router` and use `tower::ServiceExt::oneshot` with `Request::builder().method("POST").uri("/api/replay")...`, OR build the handler invocation directly:

  ```rust
  let resp = replay(State(state.clone()), Json(ReplayReq { path: file.to_string_lossy().into() })).await?;
  ```

- Construct `AppState` with a real `SqlitePool` (via `ghcp_mon::db::open`) and a fresh `Broadcaster`.
- Write a small JSONL fixture file with a couple of valid envelope lines (use a `SpanEnvelope` serialized via `serde_json::to_string` wrapped as `{"type":"span", ...}` — see [[Model Envelope Cheatsheet]] for the exact serde shape). Insert one blank line and one bad line to exercise `ingest_jsonl_file`'s skip behavior.
- Assert the JSON response shape: `{"path": <input>, "ingested": <count>}` where count equals the number of valid envelopes processed.
- Test missing-file path: pass a non-existent path and assert the resulting `AppError` (or HTTP status 400 if going through a router) carries a `"bad request:"`-prefixed message.
- For full end-to-end coverage, route this via `api_router(state)` and exercise via `oneshot` POST with `Content-Type: application/json` and a `serde_json::json!({"path": ...}).to_string()` body.
