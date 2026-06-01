---
type: LLR
tags:
  - req/llr
  - tui
  - domain/text-block
---
While in the `Active` phase, `SearchableTextBlock` MUST render a sticky header on the block's top row containing two spans, both styled `Color::DarkGray` + `Modifier::DIM`. The left span reads `"${match_index + 1} of ${match_count} matches"` (1-based) when `query` is non-empty and `match_count > 0`, and `"0 matches"` otherwise. The right span is the static keybinding hint `"(shift)+CR prev/next  Esc exit"`, right-aligned to the block's inner width (painted only when it fits without overlapping the left span). The header occupies row 0 of the block; the body begins one row below it, and the search input bar occupies the block's bottom row.

## Rationale
The counter gives immediate feedback on how many matches the query produced and the user's position in the cycle; the static hint discloses the `Enter` / `Shift+Enter` cycle and `Esc` exit keys. This is the terminal analog of the web `tb-search-header`, with `+CR` (carriage return) substituted for the web's `+LMB` (left mouse button) since cycling is keyboard-driven in the TUI.

## Derived from
- [[TextBlock search header shows match counter]]
- [[TextBlock Enter cycles Escape exits]]
