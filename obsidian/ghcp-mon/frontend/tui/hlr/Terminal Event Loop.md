---
type: HLR
tags:
  - req/hlr
  - tui
  - domain/event-loop
---
The TUI's event loop reads from a single bounded `tokio::sync::mpsc::Receiver<AppEvent>` (capacity 256). One frame per draw, not one draw per event: every pending event MUST be drained via `try_recv` before each `terminal.draw`. WebSocket envelopes are coalesced into one `WsTick` per ratatui frame interval (~16 ms) carrying the union of cache-key prefixes dirtied since the previous tick. A separate task runs the WS coalescer; another reads crossterm events in a blocking poll; another emits the 16 ms animation tick.

## Derived LLRs
- [[TUI bounded event channel with coalesced WS ticks]]
- [[TUI drain pending events before draw]]
- [[TUI coalescer accumulates dirty prefixes per frame]]
