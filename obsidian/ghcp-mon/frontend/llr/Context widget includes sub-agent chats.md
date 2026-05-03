---
type: LLR
tags:
  - req/llr
  - domain/context-growth
---
When grouping context snapshots into chart rows, the widget MUST include all chat span snapshots regardless of `invoke_agent` ancestor depth, so that sub-agent chat turns appear as their own rows alongside top-level chat turns.

## Rationale
Sub-agent context usage is meaningful on its own and should be visible in the widget. Snapshots from sub-agent chats are surfaced as distinct rows rather than folded into the parent agent's context.

## Derived from
- [[Context Growth Widget]]
