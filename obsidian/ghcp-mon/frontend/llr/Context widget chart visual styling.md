---
type: LLR
tags:
  - req/llr
  - domain/context-growth
---
The Context Growth Widget chart MUST render `input`, `output`, and `reasoning` sub-bars as blue, orange, and yellow respectively. Each bar's width MUST be capped at 28px so bars only stretch when the chart is otherwise full. The chart MUST display a numeric y-axis scale (token counts) on the left and a turn-index scale below the bars. The widget header MUST include a color key (legend) for input / sub-agent input / output / reasoning and the limit line. The hover highlight for a bar MUST be rendered as a thick yellow underbar beneath the bar (rather than as a box outline around the bar).

## Rationale
A blue / orange / yellow palette keeps the three token categories mutually distinguishable even when one category (e.g., reasoning) is null in the data. Capping bar width preserves a readable bar shape on wide displays; explicit axes and a legend make absolute token counts, turn position, and color meaning discoverable; and a yellow underbar gives a clear hover affordance without occluding the bar's stacked colors.

## Derived from
- [[Context Growth Widget]]
