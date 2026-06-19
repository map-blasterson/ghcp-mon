---
type: LLR
tags:
  - req/llr
  - tui
  - domain/chat-detail
---
In Chat Detail DELTA mode, the summary bar MUST mark each visible-frontier segment whose content is unchanged since the prior chat span with `SummarySeg.shaded = true`. The `SummaryBar` renderer MUST paint shaded segments using `summary_bar::shaded_color(seg.color)`, which multiplies each RGB channel of `Color::Rgb` by `SHADED_DARKEN_FACTOR = 0.40`; non-RGB / indexed palette colors MUST fall back to `Color::DarkGray`. Unshaded (added) segments MUST be painted with their original color.

## Rationale
Replaces the previous brighten-on-hover heuristic with a clear visual distinction between "this chat introduced this content" and "this content was already in the prior chat span".

## Derived from
- [[Chat detail DELTA diffs against prior chat span]]
- [[TUI Chat detail summary bar paints via Buffer cell_mut]]
