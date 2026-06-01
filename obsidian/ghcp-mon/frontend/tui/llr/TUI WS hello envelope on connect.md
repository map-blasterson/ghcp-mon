---
type: LLR
tags:
  - req/llr
  - tui
  - domain/live-events
---
On every successful WS open, the TUI bus MUST log an info-level message recording the URL. The first envelope from the server is its `{kind: "hello", entity: "control"}` frame; the bus MUST treat it like any other envelope (forward + invalidate-table).

## Rationale
Keeps the WS handler symmetric across kinds and avoids special-casing hello in the receive path.

## Derived from
- [[Server URL Normalization and Reconnect Status]]
- [[WS sends hello frame on connect]]
