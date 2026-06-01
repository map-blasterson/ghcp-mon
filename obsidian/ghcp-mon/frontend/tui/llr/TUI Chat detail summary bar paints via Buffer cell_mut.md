---
type: LLR
tags:
  - req/llr
  - tui
  - domain/input-breakdown
---
The chat-detail summary bar MUST be painted by writing directly to the `Buffer` via `Buffer::cell_mut` rather than via `Gauge`, `LineGauge`, or `BarChart`. Rationale: `Gauge`/`LineGauge` render a single progress value; `BarChart` assigns the same `bar_style` to every bar (per-bar colours require `BarGroup` wrapping that obscures the proportional intent). Our model is "N visible-frontier segments, each with its own bytes and its own colour, painted as one horizontal strip exactly `area.width` cells wide." The Phase 2 `context_growth` widget is the in-repo precedent for this style of paint.

The pure cell-mapping helper `allocate_widths(total, segs, width) -> Vec<u16>` MUST sum to exactly `width` whenever `total > 0`, distribute the rounding remainder to the segments with the largest fractional parts (ties broken by original index), and return all-zeros when either `total == 0` or `width == 0`. Trailing cells (when `total == 0`) MUST be filled with a dim `·` placeholder so the bar row is not blank.

## Rationale
A pure `allocate_widths` keeps the visual layout testable without spinning up a `TestBackend`. Direct cell paint is the only way to assign a distinct colour per segment with a single row.

## Derived from
- [[Chat detail summary bar proportional to visible segments]]
