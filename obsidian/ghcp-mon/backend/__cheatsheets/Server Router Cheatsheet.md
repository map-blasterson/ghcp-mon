---
type: cheatsheet
---
Source: `src/server.rs`. Crate path: `ghcp_mon::server`.

Composes the OTLP and API/WS routers and binds two TCP listeners. Depends on `crate::api`, `crate::ingest::{otlp, replay}`, `crate::ws::{Broadcaster, handler::ws_handler}`, `crate::static_assets::static_handler`.

## Extract

```rust
use axum::{routing::{get, post}, Router};
use sqlx::SqlitePool;
use std::{net::SocketAddr, path::PathBuf, sync::Arc};
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;

use crate::ws::Broadcaster;

#[derive(Clone)]
pub struct AppState {
    pub pool: SqlitePool,
    pub bus: Broadcaster,
    pub session_state_dir_override: Arc<Option<PathBuf>>,
}

/// OTLP receiver router.
pub fn otlp_router(state: AppState) -> Router;

/// API + WebSocket router.
pub fn api_router(state: AppState) -> Router;

pub async fn serve(
    state: AppState,
    otlp_addr: SocketAddr,
    api_addr: SocketAddr,
) -> anyhow::Result<()>;
```

Routes mounted by `otlp_router`:
- `POST /v1/traces`   → `crate::ingest::otlp::traces`
- `POST /v1/metrics`  → `crate::ingest::otlp::metrics`
- `POST /v1/logs`     → `crate::ingest::otlp::logs`
- Layered with `TraceLayer::new_for_http()` and `axum::extract::DefaultBodyLimit::max(64 * 1024 * 1024)` (64 MiB).

Routes mounted by `api_router`:
- `GET  /api/healthz`
- `GET  /api/sessions`
- `GET, DELETE /api/sessions/:cid`
- `GET  /api/sessions/:cid/span-tree`
- `GET  /api/sessions/:cid/contexts`
- `GET  /api/spans`
- `GET  /api/spans/:trace_id/:span_id`
- `GET  /api/traces`
- `GET  /api/traces/:trace_id`
- `GET  /api/raw`
- `POST /api/replay`
- `GET  /ws/events`
- `fallback(static_handler)`
- Layers: `CorsLayer::new().allow_origin(Any).allow_methods(Any).allow_headers(Any)`, `TraceLayer::new_for_http()`, `DefaultBodyLimit::max(64 * 1024 * 1024)`.

`serve`:
- Builds two listeners with `tokio::net::TcpListener::bind` and runs both routers concurrently via `tokio::spawn` + `tokio::try_join!`.
- Returns `anyhow::Result<()>`.

## Suggested Test Strategy

- Don't test `serve` directly (it never returns until both listeners exit). Instead, exercise `otlp_router(state)` and `api_router(state)` as `Router` values via `tower::ServiceExt::oneshot`:

  ```rust
  use tower::ServiceExt;            // for `.oneshot`
  use http_body_util::BodyExt;      // for `.collect()` (already a dev-dep)
  use axum::http::{Request, StatusCode};

  let response = api_router(state).oneshot(
      Request::builder().uri("/api/healthz").body(axum::body::Body::empty()).unwrap()
  ).await.unwrap();
  ```

  Both `tower` (with `util` feature) and `http-body-util` are listed as `[dev-dependencies]` already.

- Build `AppState` for tests from a real in-memory schema:
  - `let pool = ghcp_mon::db::open(&unique_tempfile).await?;`
  - `let bus = Broadcaster::new(64);`
  - `let session_state_dir_override = Arc::new(None);`

- LLR-aligned cases:
  - **API router exposes session and span endpoints**: hit `/api/healthz`, `/api/sessions`, `/api/spans` (etc.) via oneshot and assert 200 (or 404 with `{"error":"not found"}` for missing :cid which is still router-level success — i.e. the route is wired).
  - **CORS allows any origin**: send a request with `Origin: http://example.com` and assert response carries `access-control-allow-origin: *` (or `"*"` echoed). Check `Access-Control-Allow-Methods`/`-Headers` similarly. Also send an `OPTIONS` preflight and assert the response is acceptable (200/204) with the CORS headers.
  - **OTLP router exposes /v1/traces|metrics|logs**: oneshot `POST` with a small JSON body to each path; even if downstream returns success/empty, asserting non-404/non-405 confirms wiring.
  - **OTLP body limit 64 MiB**: send a `POST /v1/traces` with a body > 64 MiB (e.g. 64 * 1024 * 1024 + 1 zero bytes). Expect status `413 Payload Too Large` (axum's `DefaultBodyLimit` reply). For speed, send 65 MiB of `b'a'` rather than valid OTLP JSON; the body-limit reject happens before the handler runs.
- Use `axum::body::to_bytes(resp.into_body(), usize::MAX).await` to read response bodies.
- No mocks — the `AppState` collaborators (`SqlitePool`, `Broadcaster`) work fine as real instances.
