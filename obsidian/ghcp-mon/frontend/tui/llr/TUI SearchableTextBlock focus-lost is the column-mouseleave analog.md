---
type: LLR
tags:
  - req/llr
  - tui
  - domain/text-block
---
The web `TextBlock` exits search when the closest ancestor `.col-body` element fires `mouseleave` or when a `contextmenu` (right-click) fires on the block. A terminal has no hover region and no per-element mouseleave event, so the TUI port substitutes **column focus loss** as the close cousin: the host calls `SearchableTextBlock::on_focus_lost(state, external_query)` when the column owning the block loses focus, which (absent an external query) resets the phase to `Idle` and clears `query`/`match_index`/`match_count`/`scroll_top`. Right-click dismiss is deferred to mouse mode (Phase 6); when `--mouse` is enabled a right-click on the block will route to the same `on_focus_lost` path. There is no `mouseleave` analog for merely moving off the block while the column keeps focus.

## Rationale
Search is a transient, focus-scoped affordance. In the web UI leaving the column body dismisses it; in the TUI the equivalent "I've moved on" signal is the focused column changing. Mapping focus-loss to the dismiss keeps the affordance transient without inventing a spurious terminal hover model.

## Derived from
- [[TextBlock exits search on column-body leave or right-click]]
- [[Terminal Rendering Constraints]]
