---
type: LLR
tags:
  - req/llr
  - domain/context-growth
---
The chart MUST render one stacked column per chat turn (one bar per `span_pk`), with stacked sub-bars for `cache_read`, fresh `input`, `output`, and `reasoning` token counts (see [[Context widget cache read green segment]] for the per-bar split and stacking order). The chart MUST overlay a dotted yellow horizontal limit line at the maximum `token_limit` observed across rows (rows without a `token_limit` do not contribute to that maximum). The y-axis MUST be anchored to context-window occupancy, not bar height: `maxCurrent` is the maximum `current_tokens` observed across `usage_info_event` snapshots, and `maxTokenLimit` is the maximum non-null `token_limit` observed across snapshots (zero if none was reported). The y-axis MUST be `1.10 * max(maxTokenLimit, maxCurrent)`, falling back to `1` as a guard only when both are zero (i.e., no snapshot carries either field). Bars whose stacked total exceeds the y-axis (e.g., sub-agent chats with very large per-call prompt sizes) MUST clip at the top of the plot rather than rescale the axis.

## Rationale
Anchoring the y-axis to `1.10 * max(maxTokenLimit, maxCurrent)` keeps ~10% headroom above whichever signal is larger. When `token_limit` is reported, this keeps the dotted yellow limit line visible above the bars; when no snapshot carries a `token_limit` (a common case for some providers) `maxCurrent` alone drives the axis, and the 10% factor still preserves headroom — the previous formula gave zero headroom in that case and clipped the tallest bar against the top of the plot. Using `current_tokens` (context-window occupancy) for `maxCurrent` — rather than the tallest bar — keeps the scale meaningful for the parent agent even when sub-agents have prompt sizes in the millions of tokens (sub-agents replay full conversation history per call and don't emit `usage_info_event` snapshots, so their bars dwarf the limit if used as the y-axis anchor). Letting outlier sub-agent bars clip preserves a useful scale for the rest of the chart.

## Derived from
- [[Context Growth Widget]]
