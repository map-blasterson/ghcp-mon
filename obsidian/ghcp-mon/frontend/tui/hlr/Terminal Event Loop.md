---
type: HLR
tags:
  - req/hlr
  - tui
  - domain/event-loop
---
The TUI's event loop is **event-driven** — no heartbeat tick. A single `tokio::select!` races over five sources: the WS envelope broadcast, the WS status broadcast, the async crossterm `EventStream`, the cache `Notify` ("value changed"), and one optional animation deadline whose value is the min of the next reveal-queue head and the next 250 ms boundary (gated on whether the most recent draw actually painted a spinner). After the first arm resolves, any WS envelopes (and status updates) that arrived during processing are drained via `try_recv` and the WS batch is applied with a **single union-of-prefixes cache invalidation** before exactly one `terminal.draw`. A fully idle TUI parks indefinitely.

## Derived LLRs
- [[TUI event loop is tokio select over five sources]]
- [[TUI drain pending events before draw]]
- [[TUI batch WS invalidation per loop cycle]]
- [[TUI animation deadline gates on spinner_visible]]
- [[TUI follow-mode advances on cache changed]]
