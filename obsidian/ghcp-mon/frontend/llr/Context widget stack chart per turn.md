---
type: LLR
tags:
  - req/llr
  - domain/context-growth
---
The chart MUST render one stacked column per chat turn (one bar per `span_pk`), with stacked sub-bars for `input`, `output`, and `reasoning` token counts. The `input` sub-bar MUST exclude `cache_read_tokens` for sub-agent chats (i.e., bar input = `max(0, input_tokens - cache_read_tokens)` when the chat span's `invoke_agent` ancestor depth is greater than 1) so the chart shows only fresh, non-cached input tokens for sub-agent calls. For the root agent (depth ≤ 1), the `input` sub-bar MUST use `input_tokens` as-is (cache reads included). The chart MUST overlay a dotted yellow horizontal limit line at the maximum `token_limit` observed across rows (rows without a `token_limit` do not contribute to that maximum). The y-axis MUST be anchored to context-window occupancy, not bar height: `maxCurrent` is the maximum `current_tokens` observed across `usage_info_event` snapshots. When at least one snapshot carries a `token_limit`, the y-axis MUST be `max(maxTokenLimit * 1.10, maxCurrent)`; when no snapshot carries a `token_limit`, the y-axis MUST be `maxCurrent` (or `1` as a guard when no usage_info_event snapshot exists). Bars whose stacked total exceeds the y-axis (e.g., sub-agent chats with very large per-call prompt sizes) MUST clip at the top of the plot rather than rescale the axis.

## Rationale
Anchoring the y-axis to `1.10 * maxTokenLimit` keeps the dotted yellow limit line visible with ~10% headroom above it. Using `current_tokens` (context-window occupancy) for `maxCurrent` — rather than the tallest bar — keeps the scale meaningful for the parent agent even when sub-agents have prompt sizes in the millions of tokens (sub-agents replay full conversation history per call and don't emit `usage_info_event` snapshots, so their bars dwarf the limit if used as the y-axis anchor). Letting outlier sub-agent bars clip preserves a useful scale for the rest of the chart.

## Derived from
- [[Context Growth Widget]]
