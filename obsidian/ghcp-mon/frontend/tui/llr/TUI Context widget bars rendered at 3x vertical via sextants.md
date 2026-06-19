---
type: LLR
tags:
  - req/llr
  - tui
  - domain/context-widget
---
The Context Growth Widget MUST paint bar cells at **3× vertical resolution** using the Unicode 13.0 Legacy Computing sextant block (`ratatui::symbols::pixel::SEXTANTS`). Per cell the renderer MUST compute a fractional fill `clamp(c4 - rr, 0, 1)` and round to one of {0, 1, 2, 3} sub-rows out of `SUBROWS_PER_CELL = 3`. Cells fully inside the stack MUST render as `█` and preserve the existing center-rule segment color. Partial top-of-stack cells MUST render the matching glyph from `PARTIAL_FILL_BITS = [0b000000, 0b110000, 0b111100, 0b111111]` in the topmost-present segment's color, so segments shorter than 1 cell (common when `token_limit ≫ tokens`) remain visible instead of being center-ruled away.

## Rationale
Tiny `cache_read`/`reasoning` segments were invisible under the previous full-block-only renderer; sextants triple the achievable vertical resolution without changing the layout.

## Derived from
- [[Context widget chart visual styling]]
- [[TUI Context widget bar cell layout and tail truncation]]
