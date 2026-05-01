---
type: LLR
tags:
  - req/llr
  - domain/context-growth
---
Dragging the resizer at the top of the widget MUST update `contextWidgetHeightVh` to `clamp(startVh - (dy / window.innerHeight) * 100, 5, 80)`, where `dy` is the pointer's vertical delta from drag start.

## Rationale
Clamping prevents the user from collapsing the widget out of reach or expanding it past the viewport.

## Derived from
- [[Context Growth Widget]]
