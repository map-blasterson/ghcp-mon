---
type: impl
source: src/tui/scenarios/scenario.rs
lang: rust
tags:
  - impl/original
  - impl/rust
---
Defines the `Scenario` trait, the per-call `Ctx` (disjoint borrows of App: api, cache, workspace, hovered_chat_pk, span_detail_memo), `KeyOutcome { consumed, effects }`, `WsBatchMeta { touches_spans }`, and Ctx's semantic cache helpers (`cached_sessions`, `cached_traces` / `cached_traces_readonly`, `cached_session_span_tree`, `session_span_tree_loaded`, `cached_session_contexts`, `cached_span_detail` with generation-keyed memo, `cached_search_hits` ReadOnly, `kick_search_debounce` 300 ms). Trait methods: `draw`, `handle_key`, `keymap_entries`, `on_ws_batch` (default no-op), `on_cache_changed`, `tick`, `next_anim_deadline`, `text_input_active`, `popover_active`, `as_any`/`as_any_mut`.

## Source For
- [[TUI Scenario trait owns column behavior]]
- [[Per-Scenario Trait Architecture]]
