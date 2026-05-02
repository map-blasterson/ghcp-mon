---
type: LLR
tags:
  - req/llr
  - domain/input-breakdown
---
A primitive value rendered inside a tree node whose stringified length exceeds `200` characters or contains a newline MUST be rendered through `<TextBlock truncatable text=… open=…>` (which applies the `ib-prim-v-clip` class and the `open` modifier), and `NodeView` MUST drive the `open` state via a per-node `expandedPrims` set keyed by `${node.id}__p${i}` toggled by clicks on the corresponding `.ib-prim-k` key span (carrying the `clickable` class). Clicks on the key span MUST call `e.stopPropagation()` so they do not collapse the parent node.

## Rationale
The click target lives on the key span (so the value text remains free for selection / per-block search), and the value block itself is purely a controlled view via the `open` prop.

## Derived from
- [[Chat detail]]
