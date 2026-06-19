---
type: LLR
tags:
  - req/llr
  - tui
  - domain/text-block
---
After `locate_matches` runs in the `Active` phase, `SearchableTextBlock` MUST clamp `state.match_index` to `match_count - 1` when it overflows (e.g. after the query shrank the result set), then adjust `state.scroll_top` so the current match's wrapped `row` falls inside the visible window `[scroll_top, scroll_top + plot_h)`, where `plot_h` is the body height in rows. The algorithm: if `current.row < scroll_top`, set `scroll_top = current.row` (scroll up to reveal); if `current.row >= scroll_top + plot_h`, set `scroll_top = current.row.saturating_sub(plot_h - 1)` (scroll down so the match sits on the last visible row); otherwise leave `scroll_top` unchanged. This runs on every render, so cycling matches with `Enter` / `Shift+Enter` (which only move `match_index`) re-scrolls on the next frame.

## Rationale
The current match must stay visible inside the block's scroll window as the user cycles or edits the query — the terminal analog of the web `scrollIntoView({ block: "nearest" })`. Re-clamping `match_index` before scrolling avoids indexing a stale, now-out-of-range match.

## Derived from
- [[TextBlock current match scrolls into view]]
- [[Terminal Rendering Constraints]]
