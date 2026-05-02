---
type: LLR
tags:
  - req/llr
  - domain/text-block
---
While in the `active` phase with `matchCount > 0`, a click on the block (outside the `.tb-search-input-wrap`) MUST advance `matchIndex` by `(i + 1) % matchCount` for a plain click and `(i - 1 + matchCount) % matchCount` when `shiftKey` is held, and MUST call `e.preventDefault()` + `e.stopPropagation()` so the cycle click does not bubble to ancestor handlers.

## Rationale
Mouse-only cycling lets the user scan matches without leaving the input or moving to keyboard.

## Derived from
- [[Searchable Text Block]]
