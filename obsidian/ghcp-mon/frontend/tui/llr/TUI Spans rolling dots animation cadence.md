---
type: LLR
tags:
  - req/llr
  - tui
  - domain/traces
---
The TUI rolling-dots indicator MUST cycle through 4 frames
(`"   "`, `".  "`, `".. "`, `"..."`), advancing one frame every
`FRAMES_PER_STEP = 4` ticks. With the 16 ms tick rate from
[[Terminal Event Loop]], one frame step ≈ 64 ms and a full cycle ≈ 256 ms.

The frame index is computed deterministically from the App's monotonic
`anim_tick` counter (incremented once per `AppEvent::Tick`) so every
renderer reading the same tick produces the same frame.

Used by:
- Spans tree rows with `ingestion_state === "placeholder"` (per
  [[Placeholder ingestion state shown with rolling dots]]).
- The Spans column's body loading placeholder when the cache has no
  session span tree yet.

## Rationale
A shared frame source lets every rolling-dots instance on a frame stay in
phase without per-widget timers.

## Derived from
- [[Placeholder ingestion state shown with rolling dots]]
- [[Terminal Event Loop]]
