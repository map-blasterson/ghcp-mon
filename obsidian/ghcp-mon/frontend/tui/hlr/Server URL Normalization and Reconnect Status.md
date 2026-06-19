---
type: HLR
tags:
  - req/hlr
  - tui
  - domain/live-events
---
The `--server` argument is normalized before any composition:

- Trailing `/` is stripped.
- Schemes other than `http` / `https` are rejected with a clear error before the TUI starts.
- The WS URL is derived by `http→ws` / `https→wss` substitution + `/ws/events` suffix.

The WS bus is a singleton with lazy `start()`. Reconnect uses exponential backoff `min(30_000, 500 * 2^attempt)` ms. After ≥5 consecutive reconnect failures the status changes from amber (`Reconnecting`) to red (`Error`); the `?` overlay shows the most recent error in its tail.

## Derived LLRs
- [[TUI URL normalization rules]]
- [[TUI bus singleton lazy start]]
- [[TUI WS reconnect exponential backoff capped at 30s]]
- [[TUI WS reconnect failure threshold transitions status to error]]
- [[TUI WS malformed JSON ignored]]
- [[TUI WS error closes socket to trigger reconnect]]
- [[TUI panic hook restores terminal]]
