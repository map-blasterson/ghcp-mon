---
type: LLR
tags:
  - req/llr
  - domain/input-breakdown
---
While the pointer hovers a truncatable `.ib-prim-k` key span, `NodeView` MUST mount a `KeyCursorIcon` component that renders `<span class="kc-cursor-icon" aria-hidden="true">` with text `[+]` when the corresponding primitive is collapsed or `[-]` when expanded, updating its `transform` on every window `mousemove` to `translate(${e.clientX + 12}px, ${e.clientY + 12}px)`. The component MUST unmount as soon as the hover ends, removing its mousemove listener.

## Rationale
Tells the user the key is the click target for expansion without committing layout space to a static affordance.

## Derived from
- [[Chat detail long primitives click to expand]]
