---
type: impl
source: src/tui/scenarios/spans/follow_mode.rs
lang: rust
tags:
  - impl/generated
  - impl/rust
---
Pure helper: walk the loaded tree for the latest `execute_tool`/`external_tool` span by `sort_key = (end_unix_ns, start_unix_ns, span_pk)`. Caller (App) toggles follow-mode and decides when to auto-advance `selected_span_id` to this result.

## Source For
- [[Spans follows latest tool span]]
