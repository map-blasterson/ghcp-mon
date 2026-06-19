---
type: LLR
tags:
  - req/llr
  - tui
  - domain/traces
---
The Spans scenario's reveal queue MUST be drained once per
`AppEvent::Tick`. The draining helper
(`reveal_schedule::RevealState::drain_due(now_ms)`) moves any
`(span_id, reveal_at_ms)` entry whose `reveal_at_ms <= now_ms` from the
queue into `revealed_ids` and returns the span_ids newly revealed. The
App MUST then re-render so the new spans appear in the tree.

Because the tick rate is 16 ms (per [[Terminal Event Loop]]) and the
schedule's global `1000/60` ms minimum gap matches it exactly, the queue
drains at most one batch per frame, giving the user the spec-mandated
60-reveal/sec cap.

When the column's `session` changes, the App MUST synchronously call
`reveal_schedule::reset()` to clear `revealed_ids`, the queue, and any
pending entries — restoring the first-batch-immediate behavior for the
new session.

## Rationale
A per-tick drain integrates the smoothing scheduler with the App's
drain-then-draw event loop without spawning per-span timers.

## Derived from
- [[Spans batch arrival smoothing]]
- [[Terminal Event Loop]]
