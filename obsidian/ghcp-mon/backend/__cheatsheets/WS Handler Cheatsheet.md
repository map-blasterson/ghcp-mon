---
type: cheatsheet
---
Source: `src/ws/handler.rs`. Crate path: `ghcp_mon::ws::handler`.

Depends on: [[Broadcaster Cheatsheet]] (`EventMsg`, `Broadcaster`), and `crate::server::AppState`.

## Extract

```rust
use axum::{
    extract::{ws::{Message, WebSocket, WebSocketUpgrade}, State},
    response::IntoResponse,
};
use crate::server::AppState;

pub async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> impl IntoResponse;

// private:
// async fn client_loop(mut socket: WebSocket, state: AppState);
```

`AppState` (from `src/server.rs`):

```rust
#[derive(Clone)]
pub struct AppState {
    pub pool: sqlx::SqlitePool,
    pub bus: ghcp_mon::ws::Broadcaster,
    pub session_state_dir_override: std::sync::Arc<Option<std::path::PathBuf>>,
}
```

Behavioral seams (observable from tests, no body details beyond what the LLR claims):
- On connect, the handler sends one text frame containing JSON `{"kind":"hello","entity":"control","payload":{"server":"ghcp-mon"}}`.
- It subscribes to `state.bus` and forwards each received `EventMsg` as a text frame containing `serde_json::to_string(&m)`.
- Incoming `Message::Ping(p)` → handler replies with `Message::Pong(p)` (echoes the payload).
- Incoming `Message::Close(_)` or `recv()` returning `None` → handler exits the loop (closes the socket).
- `RecvError::Lagged(n)` is logged at debug; the loop continues.
- `RecvError::Closed` → the loop exits.

## Suggested Test Strategy

- Wire the handler into a small `axum::Router` and serve it on an ephemeral TCP port (`tokio::net::TcpListener::bind("127.0.0.1:0")`) inside the test, then connect with a real WebSocket client.
  - **Gotcha:** `tokio-tungstenite` is *not* in dev-deps. Either add it ad-hoc as a dev-dep, or use `axum-test` / a hand-rolled raw TCP+WS handshake. Easiest is to add `tokio-tungstenite = "0.21"` (or any compatible) as a dev-dep.
  - Build `AppState` with a real pool from `ghcp_mon::db::open(&tempfile)` and a fresh `Broadcaster::new(64)`.
- Test cases map directly to LLRs:
  - **Hello on connect**: connect, read first frame, assert it equals the literal JSON above (or parse and assert fields).
  - **Forwards broadcast events**: after connect, call `state.bus.send(EventMsg::raw("span", json!({"x":1})))`; the next frame the client receives equals `serde_json::to_string(&msg)`.
  - **Ping → pong**: client sends `Ping(b"abc")`; client should observe `Pong(b"abc")`. Note: tungstenite typically auto-replies to pings client-side — drive it manually using `Message::Ping`/disable auto-pong, or test the *server* reply by sending an explicit ping frame and reading raw frames.
  - **Closes on client close**: client sends `Close` and drops; the server task should exit cleanly. Verify by checking that subsequent broadcasts no longer reach a clone of `bus.subscribe()` slot used by this handler (or simply that no panic / hang occurs).
- Keep tests under `#[tokio::test(flavor = "multi_thread")]` so the spawned server and client can both make progress.
- Don't mock `Broadcaster` — instantiate it. `AppState` is `Clone`, share it between the spawned server task and the test thread.
