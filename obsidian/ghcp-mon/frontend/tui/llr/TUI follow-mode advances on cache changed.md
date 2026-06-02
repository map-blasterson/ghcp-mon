---
type: LLR
tags:
  - req/llr
  - tui
  - domain/event-loop
---
When the query cache emits a "value changed" `Notify` wakeup (background fetch completion or external invalidation), the event loop MUST invoke `App::dispatch_cache_changed`, which in turn MUST call `Scenario::on_cache_changed` on every scenario in workspace order so any engaged follow-mode column re-runs its latest-tool-span walk against the freshly arrived `["session-span-tree", cid]` value. The dispatch MUST be idempotent for unchanged trees.

## Rationale
Without this, follow-mode lands one tool span behind because the walk runs against the stale cached tree before the new envelope's fetched value arrives.

## Derived from
- [[Terminal Event Loop]]
- [[Async Query Cache]]
