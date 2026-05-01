---
type: LLR
tags:
  - req/llr
  - domain/traces
---
`SpansScenario` MUST subscribe to `useLiveFeed` with the seven envelope filters `(trace, trace)`, `(span, span)`, `(span, placeholder)`, `(derived, tool_call)`, `(derived, chat_turn)`, `(derived, agent_run)`, `(derived, session)`; on every `tick` change it MUST invalidate the `["sessions"]` query and either the `["session-span-tree", session]` query when `session` is set, or the `["traces"]` query otherwise.

## Rationale
Any of these envelope classes may change which spans belong to a session; invalidating on each keeps the tree authoritative.

## Derived from
- [[Trace and Span Explorer]]
