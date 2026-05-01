---
type: cheatsheet
---
Source: `src/ws/mod.rs`. Crate path: `ghcp_mon::ws`.

## Extract

```rust
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::broadcast;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EventMsg {
    pub kind: String,    // "span" | "metric" | "log" | "derived" | "trace"
    pub entity: String,  // free-form: "tool_call", "turn", "session", "placeholder", ...
    pub payload: Value,
}

impl EventMsg {
    pub fn raw(kind: &str, payload: Value) -> Self;     // entity = kind
    pub fn derived(entity: &str, payload: Value) -> Self; // kind = "derived"
}

#[derive(Clone)]
pub struct Broadcaster { /* tx: broadcast::Sender<EventMsg> */ }

impl Broadcaster {
    pub fn new(cap: usize) -> Self;
    pub fn subscribe(&self) -> tokio::sync::broadcast::Receiver<EventMsg>;
    pub fn send(&self, msg: EventMsg);   // ignores send errors (no-receiver case)
}

pub mod handler;
```

`Broadcaster` is `Clone`; cloning shares the underlying `tokio::sync::broadcast::Sender`. `send` does **not** error when there are zero receivers.

## Suggested Test Strategy

- `#[tokio::test]` (broadcast channel needs the runtime).
- Direct exercises:
  - `Broadcaster::new(N)` then `subscribe()` once → `send(msg)` → `rx.recv().await` returns the same `EventMsg` (compare via `Debug`/`serde_json::to_value` or by field).
  - Send with **no** subscribers — must not panic and must not return an error from the public API (`send` returns `()`).
  - Two subscribers each receive the same message (broadcast fan-out). Both `recv` complete before any further `send`.
  - Constructor `EventMsg::raw("span", payload)` → `kind == entity == "span"`. `EventMsg::derived("session", payload)` → `kind == "derived"`, `entity == "session"`.
  - Capacity: send `cap + k` items without consuming, then a slow receiver should observe `RecvError::Lagged(n)` per `tokio::sync::broadcast::error::RecvError`. (Optional — only if your LLR mentions lag handling.)
- No mocks. The seam **is** `Broadcaster`; downstream code (e.g. `ws::handler`) takes `&Broadcaster` (or `state.bus`) directly and calls `subscribe()`.
- `EventMsg` is `Serialize + Deserialize` — verify JSON shape: `{"kind": "...", "entity": "...", "payload": <value>}`.
