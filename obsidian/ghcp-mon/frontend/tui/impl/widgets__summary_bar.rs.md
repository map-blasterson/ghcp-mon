---
type: impl
source: src/tui/widgets/summary_bar.rs
lang: rust
tags:
  - impl/generated
  - impl/rust
---
Proportional segment bar widget for the Chat Detail summary row. `SummarySeg { id, bytes, color, label }`; `SummaryBar { segments, hovered }`; `render(area, buf)` paints `█` cells via `Buffer::cell_mut` (the precedent set by `context_growth.rs`), brightening the hovered segment with BOLD | REVERSED. The pure helper `allocate_widths(total, segs, width)` rounds-down each segment's proportional share then distributes the rounding remainder to the largest fractional parts (ties broken by original index) so the result sums to exactly `width`. `total == 0` and `width == 0` both yield all-zeros (the renderer then fills the bar with a dim `·` placeholder). Tested via pure-logic tests for `allocate_widths` and rendering tests that assert ≥ 2 distinct colours and that hover BOLDS/REVERSES at least one cell.

## Source For
- [[Chat detail summary bar proportional to visible segments]]
- [[TUI Chat detail summary bar paints via Buffer cell_mut]]
