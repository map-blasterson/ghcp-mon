---
type: LLR
tags:
  - req/llr
  - tui
  - domain/event-loop
---
`App::next_anim_deadline_ms(spinner_visible)` MUST return the earliest wall-clock millisecond at which the event loop must wake to advance an animation, computed as the minimum of (a) the earliest `Scenario::next_anim_deadline` across all columns, and (b) the next `rolling_dots::FRAME_STEP_MS` (= 250 ms) boundary IFF `spinner_visible` is true. When neither component is active, it MUST return `None`. The `spinner_visible` input MUST come from the renderer's `DrawOutcome` (i.e. a frame was actually painted with a spinner), not from cache in-flight state.

## Rationale
Placeholder span rows animate without any fetch in flight, and empty-result fetches terminate but still leave a "loading…" line on screen; gating on `DrawOutcome` is ground truth.

## Derived from
- [[Terminal Event Loop]]
