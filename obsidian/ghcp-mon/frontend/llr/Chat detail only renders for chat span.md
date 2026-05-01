---
type: LLR
tags:
  - req/llr
  - domain/input-breakdown
---
`ChatDetailScenario` MUST treat the selected span's content as a chat input only when `detail.span.kind_class === "chat"`; for any other `kind_class` it MUST render the empty state `"selected span is not a chat span"` and MUST NOT build the breakdown tree.

## Rationale
Only inference / chat / agent spans carry the GenAI content attributes that drive the breakdown.

## Derived from
- [[Chat detail]]
