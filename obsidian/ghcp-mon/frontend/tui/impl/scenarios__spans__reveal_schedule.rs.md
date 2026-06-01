---
type: impl
source: src/tui/scenarios/spans/reveal_schedule.rs
lang: rust
tags:
  - impl/generated
  - impl/rust
---
Pure-function reveal scheduler implementing the 5-clause batch arrival smoothing rule: first-batch immediate, 2 s newest-first window, post-order hierarchy clamp, 1000/60 ms global min-gap, session-switch reset. RevealState carries `revealed_ids` + the pending `(span_id, reveal_at_ms)` queue; `drain_due(now_ms)` advances the queue on each tick.

## Source For
- [[Spans batch arrival smoothing]]
- [[TUI Reveal schedule advances on every tick]]
