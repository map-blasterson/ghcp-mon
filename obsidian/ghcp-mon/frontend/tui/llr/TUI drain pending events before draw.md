---
type: LLR
tags:
  - req/llr
  - tui
  - domain/event-loop
---
After the first `tokio::select!` arm resolves, the TUI event loop MUST drain every pending WebSocket envelope (`ws_rx.try_recv`) and every pending status update (`status_rx.try_recv`) before redrawing, MUST apply the drained WS envelopes via a single `App::on_ws_envelopes(batch)` call, and MUST call `terminal.draw` at most once per cycle (only when `dirty` is true).

## Rationale
One frame per draw, not one draw per event — a burst of N envelopes collapses into one cache-invalidation scan and one repaint.

## Derived from
- [[Terminal Event Loop]]
