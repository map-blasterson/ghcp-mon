---
type: impl
source: src/tui/scenarios/spans/mod.rs
lang: rust
tags:
  - impl/generated
  - impl/rust
---
Spans scenario top-level: SpansState (cursor, user_collapsed, follow_mode, search, reveal state); selection_allow per kind class; propagate_selection (with direct-chat tool-call-id clear + execute_tool auto-advance to chat); propagate_search to detail columns; hovered_chat_ancestor lookup; submodule re-exports.

## Source For
- [[Span selection routes by kind class allow list]]
- [[Spans direct chat selection clears tool call hint]]
- [[Spans search propagates query to detail columns]]
- [[Spans scenario two modes session vs traces]]
- [[Spans live invalidation on ingest events]]
- [[Spans header collapse and expand all buttons]]
- [[Span tree row publishes hovered chat ancestor]]
- [[TUI Spans focused row publishes hovered chat ancestor]]
- [[Context widget bar click selects chat in Spans column]]
- [[TUI Spans tree row suppresses noisy names]]
