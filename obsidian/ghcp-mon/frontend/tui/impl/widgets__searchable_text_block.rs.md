---
type: impl
source: src/tui/widgets/searchable_text_block.rs
lang: rust
tags:
  - impl/generated
  - impl/rust
---
Reusable `SearchableTextBlock` ratatui widget + `SearchableTextBlockState`: terminal port of the web `TextBlock` search affordance. Phases `Idle`/`Icon`/`Active`; `[?]` corner hint on focus; sticky match-counter header `"N of M matches"` + `"(shift)+CR prev/next  Esc exit"`; case-insensitive non-overlapping match highlighting (plain yellow / brighter-yellow-bold current) computed over the **wrapped** glyph stream via the pure `wrap_text` / `locate_matches` helpers; scroll-into-view on cycle; controlled `truncatable && !open` ellipsis mode that still counts hidden matches; `external_query` programmatic lifecycle that suppresses interactive Esc / focus-loss exit. Reuses Phase 1's `search_input::SearchInput`/`SearchInputView` for the bottom input bar. Demo: `cargo run --example searchable_text_block`.

## Source For
- [[TextBlock cursor-following search hint icon]]
- [[TextBlock click activates search input]]
- [[TextBlock highlights case-insensitive matches]]
- [[TextBlock click cycles matches shift for previous]]
- [[TextBlock Enter cycles Escape exits]]
- [[TextBlock exits search on column-body leave or right-click]]
- [[TextBlock current match scrolls into view]]
- [[TextBlock truncatable controlled by open prop]]
- [[TextBlock search header shows match counter]]
- [[TextBlock accepts external search query prop]]
- [[TextBlock external query suppresses interactive exit gestures]]
- [[TUI SearchableTextBlock static question-mark hint replaces cursor-follow glyph]]
- [[TUI SearchableTextBlock wrap and locate]]
- [[TUI SearchableTextBlock scroll into view on cycle]]
- [[TUI SearchableTextBlock truncate with ellipsis on last visible row]]
- [[TUI SearchableTextBlock external query suppresses Esc and focus-loss]]
- [[TUI SearchableTextBlock focus-lost is the column-mouseleave analog]]
- [[TUI SearchableTextBlock match counter sticky header]]
