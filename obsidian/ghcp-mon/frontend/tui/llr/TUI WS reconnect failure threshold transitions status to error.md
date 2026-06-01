---
type: LLR
tags:
  - req/llr
  - tui
  - domain/live-events
---
Connection failures (the connect_async future returning Err) MUST increment a `failures` counter, reset to 0 on the next successful open; once `failures >= 5`, the bus's published status MUST become `WsStatus::Error` instead of `WsStatus::Reconnecting`.

## Rationale
Lets the top-bar status dot turn red when the server is durably unreachable, separate from a transient retry.

## Derived from
- [[Server URL Normalization and Reconnect Status]]
