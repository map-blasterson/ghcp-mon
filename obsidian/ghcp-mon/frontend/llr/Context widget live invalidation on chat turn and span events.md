---
type: LLR
tags:
  - req/llr
  - domain/context-growth
---
The widget MUST subscribe to `useLiveFeed([{ kind: "derived", entity: "chat_turn" }, { kind: "span", entity: "span" }, { kind: "span", entity: "placeholder" }])` and, on each `tick`, MUST invalidate both `["session-contexts", session]` and `["session-span-tree", session]` queries when `session` is set.

## Rationale
The chart depends on snapshot and tree state; chat-turn and span events advance either source.

## Derived from
- [[Context Growth Widget]]
