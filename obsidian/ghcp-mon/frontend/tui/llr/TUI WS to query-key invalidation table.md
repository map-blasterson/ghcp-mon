---
type: LLR
tags:
  - req/llr
  - tui
  - domain/cache
---
Given a WS envelope with `(kind, entity)`, `ws_invalidation_prefixes` MUST return the literal set of cache-key prefixes per the table in §"Query cache contract" of the session plan: `derived/session` and `derived/chat_turn` invalidate `["sessions"]`; `trace/trace`, `span/span`, `span/placeholder`, and any `derived/*` invalidate `["traces"]` and `["session-span-tree"]`; `derived/chat_turn`, `span/span`, `span/placeholder` invalidate `["session-contexts"]`; `span/span` and `derived/tool_call` invalidate `["spans"]`; `span/span` and `metric/metric` invalidate `["raw"]`; `span/span` invalidates `["traces-detail"]`.

## Rationale
Encodes the exact dependency edges from the WS envelope namespace into the cache key namespace.

## Derived from
- [[Async Query Cache]]
