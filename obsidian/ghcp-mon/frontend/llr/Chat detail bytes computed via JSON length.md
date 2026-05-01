---
type: LLR
tags:
  - req/llr
  - domain/input-breakdown
---
Each tree node's `bytes` field MUST equal `JSON.stringify(value ?? null).length`, falling back to `0` if `JSON.stringify` throws. The root node's `bytes` MUST equal the sum of its four children's `bytes`.

## Rationale
Bytes are the visible quantity in the summary bar; cycles must not crash the tree build.

## Derived from
- [[Chat detail]]
