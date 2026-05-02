---
type: LLR
tags:
  - req/llr
  - domain/input-breakdown
---
When the chat span carries a `gen_ai.conversation.id` and `mode === "DELTA"`, `ChatDetailScenario` MUST locate the prior chat span — the chat-kind span immediately preceding the selected one in `(end_unix_ns ?? start_unix_ns ?? 0, span_pk)` ascending order across the whole `getSessionSpanTree(cid)` — and use its parsed `system_instructions` and `gen_ai.tool.definitions` as the `prior` baseline passed to `buildTree`. When no prior chat span exists, when its captured content is empty, or when no conversation id is set, `buildTree` MUST be called with `prior: null` (DELTA degrades to FULL).

## Rationale
Per-turn delta view requires the immediately-preceding turn from the same conversation; cross-trace siblings under one session_id are the right scope.

## Derived from
- [[Chat detail]]
