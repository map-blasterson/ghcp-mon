---
type: LLR
tags:
  - req/llr
  - domain/text-block
---
While `searchable` is truthy and the pointer is hovering the block (idle → icon phase) and search is not yet active, `TextBlock` MUST render a decorative `<span class="tb-search-icon" aria-hidden="true">?/</span>` whose `transform` is updated on every `mousemove` of the wrapper to `translate(${e.clientX - rect.left + 12}px, ${e.clientY - rect.top + 12}px)`, where `rect` is the wrapper's bounding rect.

## Rationale
The 12px lead lets the icon trail the cursor without intercepting clicks; `pointer-events: none` styling plus `aria-hidden` keep it off the input and accessibility surfaces.

## Derived from
- [[Searchable Text Block]]
