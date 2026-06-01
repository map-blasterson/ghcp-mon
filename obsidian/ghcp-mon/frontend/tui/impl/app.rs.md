---
type: impl
source: src/tui/app.rs
lang: rust
tags:
  - impl/generated
  - impl/rust
---
Top-level App: workspace, focus, log-overlay visibility, mouse bit; key dispatcher; drain-then-draw event loop; top-bar + workspace renderer.
Phase 1 additions: per-column `LiveSessionsState` / `SpansState`; cross-column `hovered_chat_pk: Arc<RwLock<Option<i64>>>`; `ConfirmModalState` for the delete-session prompt; `anim_tick` for rolling-dots; scenario key dispatch with the text-input > modal > column > global precedence; `spans_pick` realizes selection routing (via `propagate_selection`, `follow_chat::find_following_chat_span`, `invoke_agent::latest_chat_descendant`) and auto-engages/disengages follow-mode; `tick_follow_mode_advance` auto-advances the cursor + selection on each `Tick`; `cached_sessions` / `cached_session_tree` / `cached_traces` / `cached_span_detail` / `cached_search_hits` are stale-while-revalidate wrappers over the shared cache; `kick_search_debounce` spawns a 300 ms debounced server-side search; `s` / `k` keys open `SpansPopover` overlays (session selector + kind filter); span detail inspector pane renders at the bottom of the Spans column when body height >= 12 rows; chips are computed per-row (`compute_row_chips`) from cached `["span", trace_id, span_id]` detail, and report_intent titles are pre-computed per parent (`compute_report_intent_titles`); traces-list mode renders when no session is set.

## Source For
- [[Terminal Event Loop]]
- [[TUI drain pending events before draw]]
- [[TUI top bar exposes WS status dot]]
- [[TUI top bar appends column via 'a' keystroke]]
- [[TUI top bar removes focused column via 'x' keystroke]]
- [[TUI top bar shows hint string for global keys]]
- [[TUI empty workspace renders hint text]]
- [[TUI terminal MIN_COL width 24 cells]]
- [[TUI columns below MIN collapse to ellipsis label]]
- [[TUI key-dispatch precedence text-input > modal > widget > column > global]]
- [[TUI mouse capture opt-in via flag and runtime toggle]]
- [[Empty workspace shows empty state]]
- [[Default workspace seeds four columns]]
- [[Spans two-row header grid layout]]
- [[TUI Spans header two-row layout in terminal cells]]
- [[TUI Spans tree row layout in cells]]
- [[TUI Live sessions row layout in cells]]
- [[Spans search results highlight matching nodes]]
- [[Span tree kind-filter dims non-matching rows]]
- [[Spans header collapse and expand all buttons]]
- [[Span selection routes by kind class allow list]]
- [[Spans direct chat selection clears tool call hint]]
- [[Spans execute_tool selection auto-advances chat detail]]
- [[Spans invoke_agent selection routes to latest chat descendant]]
- [[Spans search propagates query to detail columns]]
- [[Selecting session propagates to dependent columns]]
- [[Delete session confirms and clears column session]]
- [[Placeholder ingestion state shown with rolling dots]]
- [[TUI Spans rolling dots animation cadence]]
- [[TUI Reveal schedule advances on every tick]]
- [[TUI Spans focused row publishes hovered chat ancestor]]
- [[Spans follows latest tool span]]
- [[Spans scenario two modes session vs traces]]
- [[Spans live invalidation on ingest events]]
- [[Spans searchbox queries server on input]]
- [[Spans session selector propagates to dependent columns]]
- [[Spans diff stat badges on file mutation tools]]
- [[Shell command chip extracts primary words]]
- [[Skill name chip shows skill argument]]
- [[Report intent title shows on parent row]]
- [[Spans tool description inline label]]
- [[Span detail view shows parent and children]]
- [[Span detail view renders projection sub-blocks]]
- [[Span inspector fetches and renders detail]]
- [[Traces list dims rows below kind filter]]
- [[TUI Spans bottom detail pane layout]]
- [[TUI Spans traces list mode]]
