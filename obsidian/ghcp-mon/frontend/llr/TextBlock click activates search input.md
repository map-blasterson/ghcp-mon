---
type: LLR
tags:
  - req/llr
  - domain/text-block
---
When `searchable` is truthy and the block is in the `icon` hover phase, a click anywhere on the block MUST transition to the `active` phase, render `<div class="tb-search-input-wrap">` containing an `<input role="searchbox" class="tb-search-input">` (focused on activation) following the cursor with the same 12px lead, and call `e.preventDefault()` + `e.stopPropagation()` on the activating click so it does not bubble to ancestor click handlers.

## Rationale
The floating glyph is decorative (pointer-events: none); the click must land on the block itself.

## Derived from
- [[Searchable Text Block]]
