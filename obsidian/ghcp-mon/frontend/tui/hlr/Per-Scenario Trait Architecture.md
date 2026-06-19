---
type: HLR
tags:
  - req/hlr
  - tui
  - domain/architecture
---
Per-column behaviour lives behind a `Scenario` trait so the top-level `App` is reduced to event-loop wiring plus a `HashMap<String, Box<dyn Scenario>>` keyed by column id. Scenarios receive a per-call `Ctx` carrying disjoint borrows of App fields (REST client, cache, workspace, hovered-chat pk, span-detail memo) and return `KeyOutcome` (consumed + effects) or `Vec<ScenarioEffect>`. Scenarios MUST NOT mutate `Workspace`, the confirm modal, the hovered-pk pubsub, or trigger persistence — they return effects that the App applies after the per-scenario borrow ends. All six scenario types (LiveSessions, Spans, ToolDetail, ChatDetail, FileTouches, RawBrowser) are trait-migrated; the legacy `draw_workspace` borrow-laundering snapshot Vec is removed.

## Derived LLRs
- [[TUI Scenario trait owns column behavior]]
- [[TUI RawBrowser scenario is placeholder behind trait]]
