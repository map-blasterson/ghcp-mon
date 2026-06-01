---
type: impl
source: src/tui/ws.rs
lang: rust
tags:
  - impl/generated
  - impl/rust
---
Singleton WebSocket bus with exponential backoff, malformed-JSON swallow, error-closes-socket, and reconnect-failure threshold transitioning status to Error. URL normalization (http→ws, https→wss).

## Source For
- [[TUI bus singleton lazy start]]
- [[TUI WS reconnect exponential backoff capped at 30s]]
- [[TUI WS hello envelope on connect]]
- [[TUI WS malformed JSON ignored]]
- [[TUI WS error closes socket to trigger reconnect]]
- [[TUI WS reconnect failure threshold transitions status to error]]
- [[TUI URL normalization rules]]
- [[WS bus singleton lazy start]]
- [[WS reconnect with exponential backoff]]
- [[WS ignores malformed JSON messages]]
- [[WS error closes socket to trigger reconnect]]
- [[WS exposes connection status to subscribers]]
- [[WS dispatches parsed envelopes to listeners]]
