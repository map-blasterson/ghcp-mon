---
type: LLR
tags:
  - req/llr
  - domain/input-breakdown
---
`hasCapturedContent(a)` MUST return `true` iff at least one of `a["gen_ai.input.messages"]`, `a["gen_ai.output.messages"]`, or `a["gen_ai.system_instructions"]` is non-null.

## Rationale
Used by views to decide whether to render the "no content captured" hint.

## Derived from
- [[Chat detail]]
