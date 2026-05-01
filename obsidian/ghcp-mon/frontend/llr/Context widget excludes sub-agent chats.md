---
type: LLR
tags:
  - req/llr
  - domain/context-growth
---
When grouping context snapshots into chart rows, the widget MUST exclude any snapshot whose `span_pk` is not in the set of "top-level" chat span_pks computed by walking the session's span tree and admitting only chat spans whose `invoke_agent` ancestor depth is `≤ 1`.

## Rationale
Sub-agent chat turns nested under `invoke_agent task` would double-count tokens against the parent agent's context.

## Derived from
- [[Context Growth Widget]]
