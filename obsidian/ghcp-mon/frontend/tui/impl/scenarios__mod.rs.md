---
type: impl
source: src/tui/scenarios/mod.rs
lang: rust
tags:
  - impl/generated
  - impl/rust
---
Phase-0 placeholder renderer (dumps `column.config` so unimplemented columns visibly receive routed selection). Phase 1 dispatches `LiveSessions` and `Spans` to their real renderers in `app.rs`; `Tool detail` / `Chat detail` / `File touches` / `Raw` retain the placeholder.

## Source For
- [[Empty Workspace Hint]]
- [[Column body dispatches by scenario type]]
- [[Scenario registry maps scenario type to component]]
