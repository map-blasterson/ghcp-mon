---
type: LLR
tags:
  - req/llr
  - tui
  - domain/live-events
---
If `serde_json::from_str::<WsEnvelope>` fails on an incoming text frame, the TUI `WsBus` MUST swallow the error, log at `debug` level only, and MUST NOT invoke any listener or invalidate any cache prefix for that frame.

## Rationale
Malformed frames must not crash the TUI or close the connection.

## Derived from
- [[Server URL Normalization and Reconnect Status]]
- [[WS ignores malformed JSON messages]]
