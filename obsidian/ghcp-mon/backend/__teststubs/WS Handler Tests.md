---
type: test-stub
tags:
  - test/generated
---
Test file: `tests/ws_handler.rs` (uses `tokio-tungstenite` dev-dep against an ephemeral-port `axum::serve(listener, api_router)`)

Covers LLRs:
- [[WS sends hello on connect]] — `sends_hello_on_connect`.
- [[WS forwards broadcast events to client]] — `forwards_broadcast_events_to_client`.
- [[WS responds to ping with pong]] — `responds_to_ping_with_pong_echoing_payload`.
- [[WS closes on client close]] — `exits_loop_when_client_sends_close`.

Tests run with `#[tokio::test(flavor = "multi_thread", worker_threads = 2)]` so the server task and client both make progress.
