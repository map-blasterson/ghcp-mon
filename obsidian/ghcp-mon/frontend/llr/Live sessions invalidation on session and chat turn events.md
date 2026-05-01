---
type: LLR
tags:
  - req/llr
  - domain/live-sessions
---
`LiveSessionsScenario` MUST subscribe to `useLiveFeed([{ kind: "derived", entity: "session" }, { kind: "derived", entity: "chat_turn" }])` and MUST invalidate the `["sessions"]` TanStack Query whenever the live-feed `tick` advances.

## Rationale
Sessions appear / advance / refresh their counters on each chat-turn upsert; live invalidation keeps the list current without polling.

## Derived from
- [[Live Session Browser]]
