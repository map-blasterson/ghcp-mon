---
type: LLR
tags:
  - req/llr
  - domain/text-block
---
When `truncatable` is true and `text` is a string whose length is `> 200` or contains a `
`, `TextBlock` MUST render the body as `<pre>` with the `ib-prim-v-clip` class and additionally apply the `open` class iff its `open` prop is truthy; when `open` is omitted or falsy, the block MUST stay collapsed. `TextBlock` MUST NOT render any built-in chevron/expand affordance — open/close is driven entirely by the parent via the `open` prop.

## Rationale
Phase 2 of the input-breakdown refactor moved the toggle target onto the `.ib-prim-k` key span owned by the parent; the block is purely a controlled view.

## Derived from
- [[Searchable Text Block]]
- [[Chat detail long primitives click to expand]]
