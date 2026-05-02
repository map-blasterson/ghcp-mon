---
type: LLR
tags:
  - req/llr
  - domain/text-block
---
While in the `active` phase, `TextBlock` MUST exit search (reset phase to `idle`, clear `query`, zero `matchIndex` and `matchCount`) when either (a) the closest ancestor `.col-body` element fires `mouseleave`, or (b) a `contextmenu` event fires on the block or on the search input — calling `e.preventDefault()` on the right-click so the native menu does not appear.

## Rationale
Search is a transient, hover-driven affordance; leaving the column or right-clicking dismisses it without a discrete close button.

## Derived from
- [[Searchable Text Block]]
