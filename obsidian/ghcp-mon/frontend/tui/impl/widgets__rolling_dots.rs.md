---
type: impl
source: src/tui/widgets/rolling_dots.rs
lang: rust
tags:
  - impl/generated
  - impl/rust
---
4-frame rolling-dots animation (`   ` / `.  ` / `.. ` / `...`) advanced once every `FRAMES_PER_STEP = 4` ticks (≈ 256 ms full cycle at 16 ms tick). Frame index is a pure function of the App's monotonic `anim_tick`, so multiple renderers stay in phase.

## Source For
- [[Placeholder ingestion state shown with rolling dots]]
- [[TUI Spans rolling dots animation cadence]]
