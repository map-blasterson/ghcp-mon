---
type: LLR
tags:
  - req/llr
  - tui
  - domain/live-events
---
The TUI `WsBus` MUST be constructed once and reused across the app's lifetime, and `WsBus::start()` MUST open the WebSocket only on its first call; subsequent calls while the socket task exists MUST be no-ops.

## Rationale
Mirrors the web `wsBus.start()` contract; one shared connection multiplexes envelopes for every subscriber.

## Derived from
- [[Server URL Normalization and Reconnect Status]]
- [[WS bus singleton lazy start]]
