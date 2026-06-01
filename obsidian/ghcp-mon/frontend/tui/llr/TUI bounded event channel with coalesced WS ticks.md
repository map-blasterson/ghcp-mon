---
type: LLR
tags:
  - req/llr
  - tui
  - domain/event-loop
---
The TUI app event channel MUST be a `tokio::sync::mpsc::channel` of capacity 256. WS envelopes MUST NOT be sent directly to this channel; instead a coalescer task MUST accumulate envelopes + dirty cache-key prefixes between draws and emit at most one `AppEvent::WsTick` per `FRAME_INTERVAL = 16 ms` carrying the union.

## Rationale
Backpressure: a burst of envelopes cannot fan out into a burst of events that overrun the draw loop. The backend's broadcast channel already logs lag, so coalescing is the correct response on the TUI side.

## Derived from
- [[Terminal Event Loop]]
