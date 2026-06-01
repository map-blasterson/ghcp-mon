---
type: LLR
tags:
  - req/llr
  - tui
  - domain/live-events
---
After a socket close or constructor failure, the TUI `WsBus` MUST schedule a reconnect attempt after `min(30_000, 500 * 2^attempt)` milliseconds where `attempt` starts at 0 and increments on every scheduling. The `attempt` counter MUST reset to 0 on a successful open.

## Rationale
Bounded retry rate matches the webui WS bus and keeps the TUI recovering after backend restarts without hammering the server during outages.

## Derived from
- [[Server URL Normalization and Reconnect Status]]
- [[WS reconnect with exponential backoff]]
