---
type: LLR
tags:
  - req/llr
  - domain/workspace
---
No column rendered by `Workspace` SHALL be assigned fewer than `120` pixels of width regardless of its `width` weight, except in the degenerate case where the available width is itself smaller than the sum of all columns' minimums.

## Rationale
Below ~120 px column scenarios become unusable.

## Derived from
- [[Workspace lays out columns by weight with ResizeObserver]]
