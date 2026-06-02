---
type: impl
source: src/tui/scenarios/spans/scenario.rs
lang: rust
tags:
  - impl/original
  - impl/rust
---
`SpansScenario` — the `Scenario` impl for Spans. Owns one `SpansState` per column (reveal queue, cursor, user_collapsed, follow_mode, search input, search_hits, popover, traces_cursor, focused_span_id, session generation). Drives: Layer-1 text-input while search is active; Layer-2 popover while session/kind picker is open; `tick` drains reveal queue; `next_anim_deadline` returns the head; `on_ws_batch` (gated on `touches_spans`) AND `on_cache_changed` both advance follow-mode cursor to the latest tool span (fixes the "lands one tool span behind" bug). Exposes `pick_span_externally` (not on the trait — reached via `as_any_mut().downcast_mut`) so the Context Growth Widget can reposition this column's cursor and emit `PropagateSelection` effects. Arrow-key cursor moves emit `PropagateSelection` so navigation acts as selection.

## Source For
- [[TUI Scenario trait owns column behavior]]
- [[TUI Spans focused row publishes hovered chat ancestor]]
- [[TUI Spans search input edit semantics]]
- [[TUI Spans rolling dots animation cadence]]
- [[TUI Reveal schedule advances on every tick]]
- [[TUI follow-mode advances on cache changed]]
- [[TUI Spans traces list mode]]
- [[TUI Spans bottom detail pane layout]]
- [[TUI Spans tree row suppresses noisy names]]
- [[TUI Spans chat row shows text preview from messages]]
- [[TUI Spans target chip renders file basename or URL host]]
