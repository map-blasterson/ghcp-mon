---
type: LLR
tags:
  - req/llr
  - domain/context-growth
---
When `contextWidgetVisible` is false, the widget MUST render only a collapsed "▾ context growth" button that, when clicked, sets `contextWidgetVisible` to true. When visible, the widget header MUST expose a `×` button that sets it to false.

## Rationale
The widget is optional decoration; the user must be able to hide and recover it.

## Derived from
- [[Context Growth Widget]]
