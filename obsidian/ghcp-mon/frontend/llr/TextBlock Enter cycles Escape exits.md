---
type: LLR
tags:
  - req/llr
  - domain/text-block
---
While the search input is focused, pressing `Enter` MUST advance `matchIndex` (with `Shift+Enter` going backward) using the same modular cycling as the click handler, and pressing `Escape` (either inside the input or at the window level while the block is `active`) MUST exit search by resetting phase to `idle`, clearing `query`, and zeroing `matchIndex` and `matchCount`.

## Rationale
Standard search-affordance keyboard contract: Enter steps, Escape closes.

## Derived from
- [[Searchable Text Block]]
