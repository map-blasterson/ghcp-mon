---
type: LLR
tags:
  - req/llr
  - tui
  - domain/architecture
---
`tui::scenarios::scenario::Scenario` MUST define `draw`, `handle_key`, `keymap_entries`, `on_ws_batch` (default no-op), `on_cache_changed` (default no-op), `tick` (default no-op), `next_anim_deadline` (default `None`), `text_input_active` (default false), `popover_active` (default false), plus `as_any`/`as_any_mut` for downcasting. `Ctx::new` MUST be constructed inside each per-column dispatch branch (not held across the whole loop) so the workspace and span-detail-memo borrows remain disjoint. Key handlers MUST NOT spawn fetches — only render-path `swr_read` (SWR policy) and explicit debounced kickers like `Ctx::kick_search_debounce` may; read-only key paths use `cached_traces_readonly` / `cached_search_hits` (`FetchPolicy::ReadOnly`). The App's `scenario_for(ScenarioType)` MUST instantiate one scenario per workspace column type without exceptions; the placeholder fall-through arm in `draw_workspace` MUST be removed.

## Rationale
Concentrates cross-column reasoning in one place; eliminates the 7-tuple snapshot Vec and lets each scenario own its state inside its trait object.

## Derived from
- [[Per-Scenario Trait Architecture]]
