---
type: LLR
tags:
  - req/llr
  - domain/input-breakdown
---
`prettyJson(v)` MUST return `JSON.stringify(v, null, 2)` and MUST fall back to `String(v)` when stringification throws (e.g. for circular structures).

## Rationale
JSON helpers must never crash an inspector view.

## Derived from
- [[Chat Input Breakdown]]
