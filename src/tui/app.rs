//! Top-level TUI App: state, event-loop, draw.
//!
//! The event loop is event-driven (no heartbeat tick). It [`tokio::select`]s
//! over: the WS envelope broadcast, the WS status broadcast, a crossterm
//! async [`EventStream`], the cache "value changed" wakeup, and a single
//! animation wake whose deadline is the minimum of the next reveal-queue
//! head and the next 250 ms boundary (gated on cache in-flight). Any WS
//! envelopes that piled up during processing are drained via `try_recv`
//! before a single `terminal.draw`. A fully idle TUI parks indefinitely.
//!
//! [`EventStream`]: ratatui::crossterm::event::EventStream

use std::collections::HashMap;
use std::rc::Rc;
use std::sync::{Arc, RwLock};

use anyhow::Result;
use futures_util::StreamExt;
use ratatui::DefaultTerminal;
use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Widget};
use ratatui::crossterm::event::{
    DisableMouseCapture, EnableMouseCapture, EventStream, KeyCode, KeyEventKind, KeyModifiers,
};
use ratatui::crossterm::execute;
use serde_json::Value;
use tokio::sync::broadcast::error::{RecvError, TryRecvError};
use tracing::{debug, info, warn};

use crate::tui::api::ApiClient;
use crate::tui::cache::{
    FetchPolicy, QueryCache, qkey, swr_read, ws_invalidation_prefixes,
};
use crate::tui::model::{SessionSpanTreeResponse, SpanTreeExt, WsEnvelope, WsKind};
use crate::tui::persist;
use crate::tui::scenarios::live_sessions::{
    clear_session_everywhere, delete_prompt, propagate_session,
};
use crate::tui::scenarios::render_placeholder;
use crate::tui::scenarios::spans::{propagate_search, propagate_selection};
use crate::tui::widgets::confirm_modal::{ConfirmModalState, ConfirmModalView};
use crate::tui::widgets::context_growth::{
    ContextGrowthState, ContextGrowthWidget, MergedRows, chat_span_pks, max_current_tokens,
    merge_snapshots,
};
use crate::tui::widgets::keymap_overlay::KeymapOverlay;
use crate::tui::widgets::log_overlay::{LogBuffer, LogOverlay};
use crate::tui::widgets::select::{SelectPopover, SelectState};
use crate::tui::widgets::status_dot::StatusDot;
use crate::tui::workspace::{ScenarioType, Workspace};
use crate::tui::ws::{WsBus, WsStatus};

/// What the renderer learned during a single `App::draw` call. Returned to
/// the event loop so it can derive its next animation deadline from
/// ground truth — what was actually drawn — instead of an indirect oracle
/// like cache in-flight state. Add fields as more animated affordances
/// arrive.
#[derive(Debug, Default, Clone, Copy)]
pub struct DrawOutcome {
    /// At least one [`rolling_dots`] frame was rendered this draw — either
    /// the "loading spans/traces" line or a placeholder span row. Drives
    /// the loop's 250 ms re-wake.
    pub spinner_visible: bool,
}

/// Minimum cell width for a column body (per terminal-rendering-constraints
/// LLR; analog of the web's `MIN_COL_PX = 280`).
pub const MIN_COL: u16 = 24;

/// What the keyboard is currently pointed at. Replaces the previous
/// `(focused_column: Option<usize>, widget_focused: bool)` pair, where the
/// combination `widget_focused = true` AND `focused_column = Some(_)` was
/// representable but had no defined meaning. With this enum, focus has
/// exactly one target — and the `Tab` cycle, key precedence, and visual
/// border-highlight can all be derived from it without ambiguity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Focus {
    #[default]
    None,
    Column(usize),
    Widget,
}

impl Focus {
    pub fn column_idx(self) -> Option<usize> {
        match self {
            Focus::Column(i) => Some(i),
            _ => None,
        }
    }
    pub fn is_widget(self) -> bool {
        matches!(self, Focus::Widget)
    }
}

/// Per-layer key-dispatch outcome (Phase 10 KeyRouter). `Pass` means the
/// layer didn't claim the key and the next layer should try; `Consumed`
/// means the key is handled and dispatch stops.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dispatch {
    Pass,
    Consumed,
}

/// App-level modal popovers that are not scoped to a scenario column.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppPopover {
    AddColumn,
}

/// Long-lived platform handles owned by the App. The "runtime" layer:
/// outbound IO (REST `api`, WS bus, log buffer) plus the shared in-process
/// caches that scenarios read through. Mostly read-only from scenario
/// code; methods that mutate runtime state (e.g. WS reconnect) live on the
/// types themselves.
///
/// Separating this from \[`App`\]'s UI state makes the dependency direction
/// explicit — scenarios and UI logic depend on `AppRuntime`, never the
/// other way around — and is a precondition for the eventual scenario
/// trait + Ctx split.
pub struct AppRuntime {
    pub api: ApiClient,
    pub ws: WsBus,
    pub cache: Arc<QueryCache>,
    pub log_buffer: LogBuffer,
}

/// Top-level app state.
pub struct App {
    pub rt: AppRuntime,
    pub workspace: Workspace,
    pub focus: Focus,
    /// Column index to restore focus to when widget focus is released
    /// (`Esc` on the focused widget, or the widget being hidden). Updated
    /// every time `focus` becomes `Focus::Column(i)`. Per the
    /// `TUI Context widget participates in Tab focus cycle` LLR — "Esc
    /// while focused, or hiding the widget, returns focus to the columns."
    pub last_focused_column: Option<usize>,
    pub log_overlay_visible: bool,
    pub keymap_overlay_visible: bool,
    pub mouse_enabled: bool,
    pub app_popover: Option<AppPopover>,
    pub add_column_picker: SelectState,
    pub status: WsStatus,
    /// Per-column `Scenario` instances. Every column scenario type goes
    /// through the trait now.
    pub scenarios: HashMap<String, Box<dyn crate::tui::scenarios::Scenario>>,
    /// Cross-column hovered chat pk store. Spans publishes via the
    /// `SetHoveredChatPk` effect; the Context Growth Widget consumes.
    pub hovered_chat_pk: Arc<RwLock<Option<i64>>>,
    pub confirm_modal: ConfirmModalState,
    /// Records what session id the pending confirm-delete refers to (none
    /// when no confirm is open).
    pub pending_delete: Option<String>,
    /// Context Growth Widget keyboard-cursor state (Phase 2).
    pub context_widget: ContextGrowthState,
    /// Last known terminal size `(width, height)`. Updated on resize and at
    /// startup; drives the widget-height `0.8 * term_h` clamp.
    pub term_size: (u16, u16),
    /// Memo for `Ctx::cached_span_detail`. Keyed by `(trace_id, span_id)`
    /// → `(cache_generation, Rc<SpanDetail>)`. Re-deserialized only when
    /// the cache generation for the span key changes; survives across
    /// draws so repeated lookups of an unchanged span (chips,
    /// report_intent walk, inspector pane, chat-detail prior lookup)
    /// cost a single HashMap hit. Lent to scenarios via `Ctx::new`.
    pub span_detail_memo:
        HashMap<(String, String), (u64, Rc<crate::tui::model::SpanDetail>)>,
}

impl App {
    pub fn new(api: ApiClient, ws: WsBus, log_buffer: LogBuffer, mouse_enabled: bool) -> Self {
        let workspace = persist::load();
        let focus = if workspace.columns.is_empty() {
            Focus::None
        } else {
            Focus::Column(0)
        };
        let last_focused_column = focus.column_idx();
        let status = ws.status();
        let mut app = Self {
            rt: AppRuntime {
                api,
                ws,
                cache: Arc::new(QueryCache::new()),
                log_buffer,
            },
            workspace,
            status,
            focus,
            last_focused_column,
            log_overlay_visible: false,
            keymap_overlay_visible: false,
            mouse_enabled,
            app_popover: None,
            add_column_picker: SelectState::default(),
            scenarios: HashMap::new(),
            hovered_chat_pk: Arc::new(RwLock::new(None)),
            confirm_modal: ConfirmModalState::new(),
            pending_delete: None,
            context_widget: ContextGrowthState::default(),
            term_size: (0, 0),
            span_detail_memo: HashMap::new(),
        };
        app.sync_scenarios_with_workspace();
        app
    }

    /// Test-only typed accessor for the [`crate::tui::scenarios::spans::SpansScenario`]
    /// bound to `col_id`. Returns `None` when the column doesn't exist or
    /// isn't a Spans column. Used by App-integration tests that
    /// previously poked the legacy `spans_state` HashMap directly.
    #[cfg(test)]
    pub fn spans_scenario(
        &self,
        col_id: &str,
    ) -> Option<&crate::tui::scenarios::spans::SpansScenario> {
        self.scenarios
            .get(col_id)?
            .as_any()
            .downcast_ref::<crate::tui::scenarios::spans::SpansScenario>()
    }

    /// Mutable counterpart of [`Self::spans_scenario`].
    #[cfg(test)]
    pub fn spans_scenario_mut(
        &mut self,
        col_id: &str,
    ) -> Option<&mut crate::tui::scenarios::spans::SpansScenario> {
        self.scenarios
            .get_mut(col_id)?
            .as_any_mut()
            .downcast_mut::<crate::tui::scenarios::spans::SpansScenario>()
    }

    /// Instantiate the `Scenario` for a given `ScenarioType`. Returns
    /// `None` for scenario types that have no implementation (none today
    /// — all six types are trait-migrated).
    fn scenario_for(t: ScenarioType) -> Option<Box<dyn crate::tui::scenarios::Scenario>> {
        use crate::tui::scenarios as sc;
        match t {
            ScenarioType::LiveSessions => Some(Box::new(sc::live_sessions::LiveSessionsScenario::new())),
            ScenarioType::ToolDetail => Some(Box::new(sc::tool_detail::ToolDetailScenario::new())),
            ScenarioType::ChatDetail => Some(Box::new(sc::chat_detail::ChatDetailScenario::new())),
            ScenarioType::FileTouches => Some(Box::new(sc::file_touches::FileTouchesScenario::new())),
            ScenarioType::RawBrowser => Some(Box::new(sc::raw_browser::RawBrowserScenario::new())),
            ScenarioType::Spans => Some(Box::new(sc::spans::SpansScenario::new())),
        }
    }

    /// Reconcile `self.scenarios` with the current workspace columns: drop
    /// scenarios for columns that no longer exist, instantiate scenarios
    /// for columns that don't yet have one (and whose scenario type is
    /// trait-migrated). Idempotent — safe to call after any workspace
    /// mutation.
    pub fn sync_scenarios_with_workspace(&mut self) {
        use std::collections::HashSet;
        let live_ids: HashSet<String> =
            self.workspace.columns.iter().map(|c| c.id.clone()).collect();
        self.scenarios.retain(|id, _| live_ids.contains(id));
        for c in &self.workspace.columns {
            if self.scenarios.contains_key(&c.id) {
                continue;
            }
            if let Some(s) = Self::scenario_for(c.scenario_type) {
                self.scenarios.insert(c.id.clone(), s);
            }
        }
    }

    /// Set focus to a column and remember it as the widget's return target.
    fn focus_column(&mut self, i: usize) {
        self.focus = Focus::Column(i);
        self.last_focused_column = Some(i);
    }

    /// Set focus to the widget. The current `last_focused_column` is left
    /// in place so `release_widget` can return there.
    fn focus_widget(&mut self) {
        self.focus = Focus::Widget;
    }

    /// Release widget focus back to the columns (LLR: "Esc while focused,
    /// or hiding the widget, returns focus to the columns"). Restores the
    /// last focused column when it is still a valid index, else falls back
    /// to column 0, else `Focus::None`.
    fn release_widget(&mut self) {
        let n = self.workspace.columns.len();
        let restore = self
            .last_focused_column
            .filter(|&i| i < n)
            .or_else(|| (n > 0).then_some(0));
        self.focus = match restore {
            Some(i) => Focus::Column(i),
            None => Focus::None,
        };
    }

    /// Apply one WebSocket envelope (convenience wrapper around
    /// [`Self::on_ws_envelopes`]). The event loop calls the batch form
    /// directly so a burst of N envelopes triggers at most one cache scan
    /// per dirtied prefix and one follow-mode advance.
    pub fn on_ws_envelope(&mut self, env: WsEnvelope) {
        self.on_ws_envelopes(std::iter::once(env));
    }

    /// Apply a batch of WebSocket envelopes. Collects the union of
    /// invalidation prefixes (so repeats collapse), then invalidates the
    /// cache and — if any envelope touched spans — advances any active
    /// follow-mode columns and refreshes the widget's row count.
    pub fn on_ws_envelopes<I>(&mut self, envs: I)
    where
        I: IntoIterator<Item = WsEnvelope>,
    {
        use std::collections::HashSet;
        let mut prefixes: HashSet<&'static [&'static str]> = HashSet::new();
        let mut touches_spans = false;
        let mut count = 0usize;
        for env in envs {
            count += 1;
            for p in ws_invalidation_prefixes(env.kind, env.entity) {
                prefixes.insert(p);
            }
            if matches!(env.kind, WsKind::Span | WsKind::Derived | WsKind::Trace) {
                touches_spans = true;
            }
            // env intentionally dropped here — no live_feed clone, no
            // last_ws_event format!() per envelope.
        }
        if count == 0 {
            return;
        }
        for p in &prefixes {
            self.rt.cache.invalidate(p);
        }
        if touches_spans {
            self.dispatch_ws_batch(crate::tui::scenarios::WsBatchMeta { touches_spans: true });
            self.context_widget.last_visible_rows = self
                .widget_merged_context()
                .map(|(m, _, _)| m.rows.len() as u16)
                .unwrap_or(0);
        }
        debug!(
            envelopes = count,
            prefixes = prefixes.len(),
            spans = touches_spans,
            "ws batch processed"
        );
    }

    /// Drain due reveal-queue entries on every scenario. Called by the
    /// event loop when its animation deadline fires.
    pub fn tick_anim(&mut self) {
        let now_ms = self.now_ms();
        let cols: Vec<(usize, String, crate::tui::workspace::ColumnConfig)> = self
            .workspace
            .columns
            .iter()
            .enumerate()
            .map(|(i, c)| (i, c.id.clone(), c.config.clone()))
            .collect();
        let mut all_effects = Vec::new();
        for (i, col_id, cfg) in cols {
            let Some(scenario) = self.scenarios.get_mut(&col_id) else {
                continue;
            };
            let mut ctx = crate::tui::scenarios::Ctx::new(
                &self.rt.api,
                &self.rt.cache,
                &self.workspace,
                &self.hovered_chat_pk,
                &mut self.span_detail_memo,
            );
            let effects = scenario.tick(&mut ctx, i, &col_id, &cfg, now_ms);
            all_effects.extend(effects);
        }
        self.apply_effects(all_effects);
    }

    /// Earliest wall-clock deadline at which the event loop must wake to
    /// advance an animation, or `None` if nothing is animating. Combines:
    ///
    /// * the earliest pending entry across all per-column reveal queues;
    /// * the next 250 ms rolling-dots boundary, but only while
    ///   `spinner_visible` is true (i.e. the most recent draw actually
    ///   rendered a [`crate::tui::widgets::rolling_dots`] frame).
    ///
    /// The spinner flag must come from the renderer (see [`DrawOutcome`])
    /// rather than from cache in-flight state — placeholder span rows
    /// animate without any fetch in flight, and empty-result fetches
    /// terminate but still leave a "loading…" line on screen until the
    /// next WS envelope.
    pub fn next_anim_deadline_ms(&self, spinner_visible: bool) -> Option<u64> {
        let mut earliest: Option<u64> = None;
        for c in &self.workspace.columns {
            if let Some(scenario) = self.scenarios.get(&c.id) {
                if let Some(at) = scenario.next_anim_deadline(&c.config) {
                    earliest = Some(earliest.map_or(at, |e| e.min(at)));
                }
            }
        }
        if spinner_visible {
            let now = self.now_ms();
            let next_quarter =
                ((now / crate::tui::widgets::rolling_dots::FRAME_STEP_MS) + 1)
                    * crate::tui::widgets::rolling_dots::FRAME_STEP_MS;
            earliest = Some(earliest.map_or(next_quarter, |e| e.min(next_quarter)));
        }
        earliest
    }

    /// Dispatch `on_ws_batch` to every scenario in workspace order.
    /// Collects effects from all scenarios then applies them after the
    /// loop so cross-scenario effect ordering is deterministic.
    fn dispatch_ws_batch(&mut self, meta: crate::tui::scenarios::WsBatchMeta) {
        let cols: Vec<(usize, String, crate::tui::workspace::ColumnConfig)> = self
            .workspace
            .columns
            .iter()
            .enumerate()
            .map(|(i, c)| (i, c.id.clone(), c.config.clone()))
            .collect();
        let mut all_effects = Vec::new();
        for (i, col_id, cfg) in cols {
            let Some(scenario) = self.scenarios.get_mut(&col_id) else {
                continue;
            };
            let mut ctx = crate::tui::scenarios::Ctx::new(
                &self.rt.api,
                &self.rt.cache,
                &self.workspace,
                &self.hovered_chat_pk,
                &mut self.span_detail_memo,
            );
            let effects = scenario.on_ws_batch(&mut ctx, i, &col_id, &cfg, &meta);
            all_effects.extend(effects);
        }
        self.apply_effects(all_effects);
    }

    /// Dispatch `on_cache_changed` to every scenario in workspace order.
    /// Called from the event-loop `cache_changed` arm so follow-mode
    /// columns re-run their latest-tool-span walk against the freshly
    /// arrived `["session-span-tree", cid]` cache value.
    fn dispatch_cache_changed(&mut self) {
        let cols: Vec<(usize, String, crate::tui::workspace::ColumnConfig)> = self
            .workspace
            .columns
            .iter()
            .enumerate()
            .map(|(i, c)| (i, c.id.clone(), c.config.clone()))
            .collect();
        let mut all_effects = Vec::new();
        for (i, col_id, cfg) in cols {
            let Some(scenario) = self.scenarios.get_mut(&col_id) else {
                continue;
            };
            let mut ctx = crate::tui::scenarios::Ctx::new(
                &self.rt.api,
                &self.rt.cache,
                &self.workspace,
                &self.hovered_chat_pk,
                &mut self.span_detail_memo,
            );
            let effects = scenario.on_cache_changed(&mut ctx, i, &col_id, &cfg);
            all_effects.extend(effects);
        }
        self.apply_effects(all_effects);
    }

    fn now_ms(&self) -> u64 {
        use std::time::{SystemTime, UNIX_EPOCH};
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0)
    }

    fn handle_key(&mut self, k: crossterm::event::KeyEvent) -> Result<bool> {
        // Single ordered walk through the precedence layers declared by the
        // `TUI key-dispatch precedence text-input > modal > widget > column > global`
        // LLR. Each layer returns `Dispatch::Consumed` (stop) or
        // `Dispatch::Pass` (try the next layer). Quitting is only ever
        // signalled by the global layer.
        use Dispatch::*;
        if let Consumed = self.layer_text_input(k) {
            return Ok(false);
        }
        if let Consumed = self.layer_modal(k) {
            return Ok(false);
        }
        if let Consumed = self.layer_widget(k) {
            return Ok(false);
        }
        if let Consumed = self.layer_column(k) {
            return Ok(false);
        }
        self.layer_global(k)
    }

    /// Layer 1 — text-input mode. Asks the focused scenario whether it
    /// is in a text-input mode (e.g. Spans `/`-active); if so, dispatch
    /// the key to its `handle_key` and respect `KeyOutcome.consumed` —
    /// keys the scenario passes back fall through to subsequent layers
    /// (so e.g. `Tab` while typing still cycles focus, matching the
    /// pre-migration behaviour).
    fn layer_text_input(&mut self, k: crossterm::event::KeyEvent) -> Dispatch {
        let Some(i) = self.focus.column_idx() else {
            return Dispatch::Pass;
        };
        let col_id = self.workspace.columns[i].id.clone();
        let cfg = self.workspace.columns[i].config.clone();
        let active = self
            .scenarios
            .get(&col_id)
            .map(|s| s.text_input_active(&cfg))
            .unwrap_or(false);
        if !active {
            return Dispatch::Pass;
        }
        let Some(scenario) = self.scenarios.get_mut(&col_id) else {
            return Dispatch::Pass;
        };
        let mut ctx = crate::tui::scenarios::Ctx::new(
            &self.rt.api,
            &self.rt.cache,
            &self.workspace,
            &self.hovered_chat_pk,
            &mut self.span_detail_memo,
        );
        let outcome = scenario.handle_key(&mut ctx, i, &col_id, &cfg, k);
        let consumed = outcome.consumed;
        self.apply_effects(outcome.effects);
        if consumed {
            Dispatch::Consumed
        } else {
            Dispatch::Pass
        }
    }

    /// Layer 2 — modal overlays: the keymap overlay, log overlay, the
    /// confirm-delete modal, app-level popovers, and column-scoped popovers
    /// (Spans `s` / `k`). Per the `Key-Dispatch Policy` HLR: modal overlays
    /// consume matching keys. Non-matching keys are swallowed by the modal
    /// (no fall-through) to avoid e.g. `q` quitting while a delete
    /// confirmation is open.
    fn layer_modal(&mut self, k: crossterm::event::KeyEvent) -> Dispatch {
        if self.keymap_overlay_visible {
            match k.code {
                KeyCode::Char('?') | KeyCode::Esc => {
                    self.keymap_overlay_visible = false;
                }
                _ => {}
            }
            return Dispatch::Consumed;
        }
        if self.log_overlay_visible {
            match k.code {
                KeyCode::Char('~') | KeyCode::Esc => {
                    self.log_overlay_visible = false;
                }
                _ => {}
            }
            return Dispatch::Consumed;
        }
        if self.confirm_modal.open {
            if let Some(confirmed) = self.confirm_modal.handle_key(k) {
                if confirmed {
                    if let Some(cid) = self.pending_delete.take() {
                        self.do_delete_session(&cid);
                    }
                } else {
                    self.pending_delete = None;
                }
            }
            return Dispatch::Consumed;
        }
        if let Some(popover) = self.app_popover {
            self.handle_app_popover_key(popover, k);
            return Dispatch::Consumed;
        }
        if let Some(i) = self.focus.column_idx() {
            let col_id = self.workspace.columns[i].id.clone();
            let cfg = self.workspace.columns[i].config.clone();
            let popover_active = self
                .scenarios
                .get(&col_id)
                .map(|s| s.popover_active(&cfg))
                .unwrap_or(false);
            if popover_active {
                if let Some(scenario) = self.scenarios.get_mut(&col_id) {
                    let mut ctx = crate::tui::scenarios::Ctx::new(
                        &self.rt.api,
                        &self.rt.cache,
                        &self.workspace,
                        &self.hovered_chat_pk,
                        &mut self.span_detail_memo,
                    );
                    let outcome = scenario.handle_key(&mut ctx, i, &col_id, &cfg, k);
                    self.apply_effects(outcome.effects);
                }
                // Popover swallows ALL keys regardless of scenario return
                // value — non-matching keys must not fall through to quit
                // / cycle focus while a picker is open.
                return Dispatch::Consumed;
            }
        }
        Dispatch::Pass
    }

    /// App-level popover key dispatch. Non-matching keys are swallowed while
    /// the popover is open.
    fn handle_app_popover_key(
        &mut self,
        which: AppPopover,
        k: crossterm::event::KeyEvent,
    ) {
        match which {
            AppPopover::AddColumn => {
                let max = ScenarioType::all().len();
                match k.code {
                    KeyCode::Esc => self.close_app_popover(),
                    KeyCode::Up => self.add_column_picker.move_cursor(-1, max),
                    KeyCode::Down => self.add_column_picker.move_cursor(1, max),
                    KeyCode::Enter => {
                        let cursor = self.add_column_picker.cursor;
                        self.close_app_popover();
                        if let Some(&scenario_type) = ScenarioType::all().get(cursor) {
                            self.append_column(scenario_type);
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    /// Layer 3 — widget-local (Context Growth Widget focused). Per
    /// `TUI Context widget keyboard bar cursor navigation` +
    /// `TUI Context widget participates in Tab focus cycle`.
    fn layer_widget(&mut self, k: crossterm::event::KeyEvent) -> Dispatch {
        if !self.focus.is_widget() {
            return Dispatch::Pass;
        }
        match k.code {
            KeyCode::Left => {
                self.widget_move_cursor(-1);
                Dispatch::Consumed
            }
            KeyCode::Right => {
                self.widget_move_cursor(1);
                Dispatch::Consumed
            }
            KeyCode::Enter => {
                self.widget_select_current();
                Dispatch::Consumed
            }
            KeyCode::Esc => {
                self.release_widget();
                Dispatch::Consumed
            }
            // Any other key falls through to the global layer.
            _ => Dispatch::Pass,
        }
    }

    /// Layer 4 — focused column's scenario handler.
    fn layer_column(&mut self, k: crossterm::event::KeyEvent) -> Dispatch {
        let Some(i) = self.focus.column_idx() else {
            return Dispatch::Pass;
        };
        let shift_only = k.modifiers.contains(KeyModifiers::SHIFT)
            && !k.modifiers.contains(KeyModifiers::ALT)
            && !k.modifiers.contains(KeyModifiers::CONTROL);
        if shift_only {
            match k.code {
                KeyCode::Left => {
                    self.move_focused_column(-1);
                    return Dispatch::Consumed;
                }
                KeyCode::Right => {
                    self.move_focused_column(1);
                    return Dispatch::Consumed;
                }
                _ => {}
            }
        }
        if self.scenario_handle_key(i, k) {
            Dispatch::Consumed
        } else {
            Dispatch::Pass
        }
    }

    /// Layer 5 — global / workspace bindings. Returns `Ok(true)` to quit.
    fn layer_global(&mut self, k: crossterm::event::KeyEvent) -> Result<bool> {
        match (k.code, k.modifiers) {
            (KeyCode::Char('q'), m) if !m.contains(KeyModifiers::SHIFT) => {
                return Ok(true);
            }
            (KeyCode::Char('c'), m) if m.contains(KeyModifiers::CONTROL) => {
                return Ok(true);
            }
            // `c` toggles the Context Growth Widget visibility.
            (KeyCode::Char('c'), m)
                if !m.contains(KeyModifiers::CONTROL) && !m.contains(KeyModifiers::ALT) =>
            {
                self.toggle_context_widget();
            }
            // Alt+↑/↓ resize widget by 1 row; +Shift by 5 rows.
            (KeyCode::Up, m) if m.contains(KeyModifiers::ALT) => {
                let step = if m.contains(KeyModifiers::SHIFT) { 5 } else { 1 };
                self.adjust_widget_height(step);
            }
            (KeyCode::Down, m) if m.contains(KeyModifiers::ALT) => {
                let step = if m.contains(KeyModifiers::SHIFT) { 5 } else { 1 };
                self.adjust_widget_height(-step);
            }
            (KeyCode::Char('?'), _) => {
                self.keymap_overlay_visible = !self.keymap_overlay_visible;
            }
            (KeyCode::Char('~'), _) => {
                self.log_overlay_visible = !self.log_overlay_visible;
            }
            (KeyCode::Char('M'), _) => {
                self.toggle_mouse();
            }
            (KeyCode::Tab, _) => self.cycle_focus(1),
            (KeyCode::BackTab, _) => self.cycle_focus(-1),
            (KeyCode::Char('a'), _) => self.open_add_column_popover(),
            (KeyCode::Char('x'), _) => self.remove_focused_column(),
            _ => {}
        }
        Ok(false)
    }

    fn active_keymap_entries(&self) -> Vec<(String, String)> {
        // When the focused scenario is in text-input mode, surface ONLY
        // that scenario's text-input keymap (no global keys — they don't
        // apply while typing).
        if let Some(i) = self.focus.column_idx() {
            if let Some(col) = self.workspace.columns.get(i) {
                if let Some(scenario) = self.scenarios.get(&col.id) {
                    if scenario.text_input_active(&col.config) {
                        return scenario.keymap_entries(&col.config);
                    }
                }
            }
        }

        let mut entries = Self::global_keymap();
        match self.focus {
            Focus::None => {}
            Focus::Widget => entries.extend(Self::context_widget_keymap()),
            Focus::Column(i) => {
                if let Some(col) = self.workspace.columns.get(i) {
                    if let Some(scenario) = self.scenarios.get(&col.id) {
                        entries.extend(scenario.keymap_entries(&col.config));
                    }
                }
            }
        }
        entries
    }

    fn keymap_entry(key: &str, desc: &str) -> (String, String) {
        (key.to_string(), desc.to_string())
    }

    fn global_keymap() -> Vec<(String, String)> {
        vec![
            Self::keymap_entry("a", "append a column"),
            Self::keymap_entry("x", "remove focused column"),
            Self::keymap_entry("Tab / Shift-Tab", "cycle focus"),
            Self::keymap_entry("c", "toggle Context Growth Widget"),
            Self::keymap_entry("Alt+↑ / Alt+↓", "resize Context Growth Widget"),
            Self::keymap_entry("M", "toggle mouse capture"),
            Self::keymap_entry("?", "toggle keymap overlay"),
            Self::keymap_entry("~", "toggle log overlay"),
            Self::keymap_entry("q", "quit"),
            Self::keymap_entry("Ctrl-C", "quit"),
        ]
    }

    fn context_widget_keymap() -> Vec<(String, String)> {
        vec![
            Self::keymap_entry("← / →", "move widget bar cursor"),
            Self::keymap_entry("Enter", "select current bar"),
            Self::keymap_entry("Esc", "release widget focus"),
        ]
    }

    /// Dispatch a key to the focused column's scenario. Returns `true` if
    /// the key was consumed. Every scenario now goes through the trait.
    fn scenario_handle_key(&mut self, col_idx: usize, k: crossterm::event::KeyEvent) -> bool {
        let col_id = self.workspace.columns[col_idx].id.clone();
        let cfg = self.workspace.columns[col_idx].config.clone();
        let Some(scenario) = self.scenarios.get_mut(&col_id) else {
            return false;
        };
        let mut ctx = crate::tui::scenarios::Ctx::new(
            &self.rt.api,
            &self.rt.cache,
            &self.workspace,
            &self.hovered_chat_pk,
            &mut self.span_detail_memo,
        );
        let outcome = scenario.handle_key(&mut ctx, col_idx, &col_id, &cfg, k);
        self.apply_effects(outcome.effects);
        outcome.consumed
    }

    /// Apply a batch of [`ScenarioEffect`]s emitted by a scenario handler.
    /// Called after the per-scenario borrow ends, so each effect is free to
    /// touch `&mut self.workspace`, the confirm modal, the hovered-chat
    /// pubsub, etc.
    fn apply_effects(&mut self, effects: Vec<crate::tui::scenarios::ScenarioEffect>) {
        for e in effects {
            self.apply_effect(e);
        }
    }

    fn apply_effect(&mut self, e: crate::tui::scenarios::ScenarioEffect) {
        use crate::tui::scenarios::ScenarioEffect;
        match e {
            ScenarioEffect::PropagateSession { origin_col_idx, cid } => {
                propagate_session(&mut self.workspace.columns, &cid, origin_col_idx);
            }
            ScenarioEffect::ConfirmDeleteSession { cid } => {
                let (title, prompt) = delete_prompt(&cid);
                self.confirm_modal.open(title, prompt);
                self.pending_delete = Some(cid);
            }
            ScenarioEffect::PersistWorkspace => {
                let _ = persist::save(&self.workspace);
            }
            ScenarioEffect::PropagateSelection {
                picked_kind,
                picked,
                chat_route,
                origin_col_idx,
            } => {
                propagate_selection(
                    &mut self.workspace.columns,
                    picked_kind,
                    picked,
                    chat_route,
                    origin_col_idx,
                );
            }
            ScenarioEffect::PropagateSearch { query } => {
                propagate_search(&mut self.workspace.columns, &query);
            }
            ScenarioEffect::SetHoveredChatPk(pk) => {
                if let Ok(mut g) = self.hovered_chat_pk.write() {
                    *g = pk;
                }
            }
            ScenarioEffect::SetKindFilter { col_idx, value } => {
                if let Some(col) = self.workspace.columns.get_mut(col_idx) {
                    match value {
                        Some(s) => {
                            col.config
                                .insert("kind_filter".into(), toml::Value::String(s));
                        }
                        None => {
                            col.config.remove("kind_filter");
                        }
                    }
                }
            }
        }
    }

    fn do_delete_session(&mut self, cid: &str) {
        let api = self.rt.api.clone();
        let cache = self.rt.cache.clone();
        let id = cid.to_string();
        tokio::spawn(async move {
            let _ = api.delete_session(&id).await;
            cache.invalidate(&["sessions"]);
        });
        clear_session_everywhere(&mut self.workspace.columns, cid);
        let _ = persist::save(&self.workspace);
    }

    /// Read-or-fetch `["session-span-tree", cid]` for the given column. Returns
    /// empty when no session is configured or the cache is empty.
    fn cached_session_tree(&self, col_idx: usize) -> Vec<crate::tui::model::SpanNode> {
        let cfg = &self.workspace.columns[col_idx].config;
        let Some(cid) = cfg.get("session").and_then(|v| v.as_str()) else {
            return Vec::new();
        };
        self.cached_session_span_tree_by_cid(cid)
    }

    /// Read-or-fetch `["session-contexts", cid]` and return the parsed
    /// snapshot list. Stale-while-revalidate.
    fn cached_session_contexts(
        &self,
        cid: &str,
    ) -> Vec<crate::tui::model::ContextSnapshot> {
        let api = self.rt.api.clone();
        let cid_s = cid.to_string();
        swr_read::<crate::tui::model::ListSessionContextsResponse, _, _>(
            &self.rt.cache,
            qkey(["session-contexts", cid]),
            std::time::Duration::from_secs(5),
            FetchPolicy::Swr,
            move || async move {
                let r = api.list_session_contexts(&cid_s).await?;
                Ok::<Value, anyhow::Error>(serde_json::to_value(r)?)
            },
        )
        .map(|r| r.context_snapshots)
        .unwrap_or_default()
    }

    /// Resolve the column the Context Growth Widget binds to: the first column
    /// (in workspace order) whose `config.session` is set. Implements
    /// `Context widget binds to first column session`.
    fn widget_bound_column(&self) -> Option<(usize, String)> {
        for (i, c) in self.workspace.columns.iter().enumerate() {
            if let Some(s) = c.config.get("session").and_then(|v| v.as_str()) {
                if !s.is_empty() {
                    return Some((i, s.to_string()));
                }
            }
        }
        None
    }

    /// Session id the widget is bound to, if any.
    pub fn widget_session(&self) -> Option<String> {
        self.widget_bound_column().map(|(_, s)| s)
    }

    /// Build the widget's merged chart data + the bound column's span tree.
    /// Returns `None` when no column has a session.
    fn widget_merged_context(
        &self,
    ) -> Option<(MergedRows, Vec<crate::tui::model::SpanNode>, usize)> {
        let (col_idx, cid) = self.widget_bound_column()?;
        let tree = self.cached_session_tree(col_idx);
        let snaps = self.cached_session_contexts(&cid);
        let pks = chat_span_pks(&tree);
        let rows = merge_snapshots(&pks, &snaps, &tree);
        let max_current = max_current_tokens(&snaps);
        Some((
            MergedRows {
                rows,
                max_current_tokens: max_current,
            },
            tree,
            col_idx,
        ))
    }

    /// Move the widget bar cursor and publish the new hover. With no prior
    /// cursor set, lands on the **last** (newest) bar so the user's first
    /// keypress doesn't jump to a turn that's been scrolled off the chart's
    /// left edge by tail-truncation.
    fn widget_move_cursor(&mut self, delta: i32) {
        let len = self
            .widget_merged_context()
            .map(|(m, _, _)| m.rows.len())
            .unwrap_or(0);
        if len == 0 {
            self.context_widget.bar_cursor = None;
            return;
        }
        let default = (len as i32) - 1;
        let cur = self
            .context_widget
            .bar_cursor
            .map(|i| i as i32)
            .unwrap_or(default);
        let next = (cur + delta).clamp(0, len as i32 - 1) as usize;
        self.context_widget.bar_cursor = Some(next);
        self.publish_widget_hover();
    }

    /// Publish the widget's current bar to the shared `hovered_chat_pk` store
    /// so the matching Spans rows highlight in sync.
    fn publish_widget_hover(&self) {
        let pk = self.context_widget.bar_cursor.and_then(|i| {
            self.widget_merged_context()
                .and_then(|(m, _, _)| m.rows.get(i).map(|r| r.span_pk))
        });
        if let Ok(mut g) = self.hovered_chat_pk.write() {
            *g = pk;
        }
    }

    /// `Enter` on the focused widget bar → route the bar's chat span as a
    /// selection through every Spans column (`Context widget bar click
    /// selects chat in Spans column`). Synchronous: for each Spans column,
    /// downcast the scenario to [`crate::tui::scenarios::spans::SpansScenario`]
    /// and delegate to its `pick_span_externally`, which moves the row
    /// cursor and emits the standard selection-routing effects.
    fn widget_select_current(&mut self) {
        let Some(i) = self.context_widget.bar_cursor else {
            return;
        };
        let Some((merged, tree, _)) = self.widget_merged_context() else {
            return;
        };
        let Some(row) = merged.rows.get(i) else {
            return;
        };
        let Some(node) = tree.find_by_pk(row.span_pk) else {
            return;
        };
        let picked_sid = node.span_id.clone();
        let spans_cols: Vec<(usize, String)> = self
            .workspace
            .columns
            .iter()
            .enumerate()
            .filter(|(_, c)| c.scenario_type == ScenarioType::Spans)
            .map(|(i, c)| (i, c.id.clone()))
            .collect();
        let mut all_effects = Vec::new();
        for (idx, col_id) in spans_cols {
            let cfg = self.workspace.columns[idx].config.clone();
            let Some(scenario) = self.scenarios.get_mut(&col_id) else {
                continue;
            };
            let Some(spans) = scenario
                .as_any_mut()
                .downcast_mut::<crate::tui::scenarios::spans::SpansScenario>()
            else {
                continue;
            };
            let mut ctx = crate::tui::scenarios::Ctx::new(
                &self.rt.api,
                &self.rt.cache,
                &self.workspace,
                &self.hovered_chat_pk,
                &mut self.span_detail_memo,
            );
            let effects = spans.pick_span_externally(&mut ctx, idx, &cfg, &picked_sid);
            all_effects.extend(effects);
        }
        self.apply_effects(all_effects);
    }

    /// Toggle the Context Growth Widget visibility (`c` global key). When
    /// hiding while the widget is focused, focus returns to the columns per
    /// the `TUI Context widget collapsed single-row bar` LLR.
    fn toggle_context_widget(&mut self) {
        self.workspace.context_widget_visible = !self.workspace.context_widget_visible;
        if !self.workspace.context_widget_visible && self.focus.is_widget() {
            self.release_widget();
        }
        let _ = persist::save(&self.workspace);
    }

    /// Clamp a candidate widget height to `3 ..= floor(0.8 * term_h)`.
    fn clamp_widget_height(&self, h: u16) -> u16 {
        let term_h = self.term_size.1;
        let cap = if term_h == 0 {
            u16::MAX
        } else {
            ((term_h as f32) * 0.8).floor() as u16
        };
        let cap = cap.max(3);
        h.clamp(3, cap)
    }

    /// Adjust widget height by `delta` rows (signed), clamped and persisted.
    fn adjust_widget_height(&mut self, delta: i32) {
        if !self.workspace.context_widget_visible {
            return;
        }
        let cur = self.workspace.context_widget_height_rows as i32;
        let raw = (cur + delta).clamp(0, u16::MAX as i32) as u16;
        let clamped = self.clamp_widget_height(raw);
        if clamped != self.workspace.context_widget_height_rows {
            self.workspace.context_widget_height_rows = clamped;
            let _ = persist::save(&self.workspace);
        }
    }

    fn cycle_focus(&mut self, dir: i32) {
        let n = self.workspace.columns.len();
        let widget_in_cycle = self.workspace.context_widget_visible;
        // Focus slots: columns `0..n`, then an optional widget slot at index n.
        let slots = n + usize::from(widget_in_cycle);
        if slots == 0 {
            self.focus = Focus::None;
            return;
        }
        let cur = match self.focus {
            Focus::Widget => n,
            Focus::Column(i) => i.min(slots - 1),
            Focus::None => 0,
        };
        let next = ((cur as i32 + dir).rem_euclid(slots as i32)) as usize;
        if widget_in_cycle && next == n {
            self.focus_widget();
            if self.context_widget.bar_cursor.is_none() {
                self.context_widget.bar_cursor = Some(0);
            }
            self.publish_widget_hover();
        } else {
            self.focus_column(next);
        }
    }

    fn open_add_column_popover(&mut self) {
        self.add_column_picker.open(0);
        self.app_popover = Some(AppPopover::AddColumn);
    }

    fn close_app_popover(&mut self) {
        self.add_column_picker.close();
        self.app_popover = None;
    }

    fn append_column(&mut self, scenario_type: ScenarioType) {
        self.workspace.add_column(scenario_type);
        let new_idx = self.workspace.columns.len() - 1;
        Self::inherit_propagated_state(new_idx, &mut self.workspace.columns);
        self.focus_column(new_idx);
        self.sync_scenarios_with_workspace();
        let _ = persist::save(&self.workspace);
    }

    /// Cross-column state that lives in `Column.config` — `session`,
    /// `selected_*`, `search_query` — is only ever written by runtime
    /// events (picking a session, picking a span, typing a search). A
    /// column added *after* one of those events would otherwise miss the
    /// already-broadcast value entirely; the user would have to re-pick
    /// the session/span/search to refill it. Mirror each `propagate_*`
    /// allow-list and copy any existing value into the new column's
    /// config.
    ///
    /// Keep this in sync with:
    /// * [`crate::tui::scenarios::live_sessions::propagate_session`]
    /// * [`crate::tui::scenarios::spans::propagate_selection`]
    /// * [`crate::tui::scenarios::spans::propagate_search`]
    fn inherit_propagated_state(
        new_idx: usize,
        columns: &mut [crate::tui::workspace::Column],
    ) {
        if new_idx >= columns.len() {
            return;
        }
        let new_type = columns[new_idx].scenario_type;

        // First-non-empty lookup for a given config key across all OTHER
        // columns. Returns the cloned value (toml::Value is small / Arc'd
        // internally for strings — clone is cheap).
        let pick = |key: &str, columns: &[crate::tui::workspace::Column]| -> Option<toml::Value> {
            columns
                .iter()
                .enumerate()
                .find_map(|(i, c)| {
                    if i == new_idx {
                        None
                    } else {
                        c.config.get(key).cloned()
                    }
                })
        };

        // `session` — propagation set: Spans | ChatDetail | FileTouches.
        if matches!(
            new_type,
            ScenarioType::Spans | ScenarioType::ChatDetail | ScenarioType::FileTouches
        ) {
            if let Some(v) = pick("session", columns) {
                columns[new_idx].config.insert("session".into(), v);
            }
        }

        // `selected_*` — propagation set for the generic allow-list path
        // in propagate_selection: Spans | ToolDetail. ChatDetail also
        // receives these via its own routing, so include it too.
        if matches!(
            new_type,
            ScenarioType::Spans | ScenarioType::ToolDetail | ScenarioType::ChatDetail
        ) {
            for key in ["selected_trace_id", "selected_span_id", "selected_tool_call_id"] {
                if let Some(v) = pick(key, columns) {
                    columns[new_idx].config.insert(key.into(), v);
                }
            }
        }

        // `search_query` — propagation set: ChatDetail | ToolDetail.
        if matches!(new_type, ScenarioType::ChatDetail | ScenarioType::ToolDetail) {
            if let Some(v) = pick("search_query", columns) {
                columns[new_idx].config.insert("search_query".into(), v);
            }
        }
    }

    fn move_focused_column(&mut self, delta: i32) {
        let Some(from) = self.focus.column_idx() else {
            return;
        };
        let n = self.workspace.columns.len();
        if from >= n || n == 0 {
            return;
        }
        let to = ((from as i32) + delta).clamp(0, n as i32 - 1) as usize;
        if to == from {
            return;
        }
        self.workspace.move_column(from, delta);
        self.focus_column(to);
        let _ = persist::save(&self.workspace);
    }

    /// `x` global key: remove the focused column. Per the `Keybinding Matrix`
    /// the binding is column-scoped — when widget is focused (not a column)
    /// it MUST be a no-op so the user does not accidentally destroy a column
    /// they cannot see being targeted.
    fn remove_focused_column(&mut self) {
        let Some(i) = self.focus.column_idx() else {
            return;
        };
        let Some(_removed_id) = self.workspace.remove_column(i) else {
            return;
        };
        self.sync_scenarios_with_workspace();
        let n = self.workspace.columns.len();
        if n == 0 {
            self.focus = Focus::None;
            self.last_focused_column = None;
        } else {
            let next = i.min(n - 1);
            self.focus_column(next);
        }
        let _ = persist::save(&self.workspace);
    }

    fn toggle_mouse(&mut self) {
        self.mouse_enabled = !self.mouse_enabled;
        let mut out = std::io::stdout();
        if self.mouse_enabled {
            let _ = execute!(out, EnableMouseCapture);
        } else {
            let _ = execute!(out, DisableMouseCapture);
        }
        info!(enabled = self.mouse_enabled, "mouse capture toggled");
    }

    pub fn draw(&mut self, frame: &mut ratatui::Frame<'_>) -> DrawOutcome {
        // span_detail_memo is generation-keyed; it self-invalidates on
        // cache changes, so we no longer clear it per frame.
        let mut outcome = DrawOutcome::default();
        let area = frame.area();
        // Reserve the widget strip at the bottom: the clamped height when the
        // widget is visible, or a single collapsed bar when hidden.
        let widget_h: u16 = if self.workspace.context_widget_visible {
            self.clamp_widget_height(self.workspace.context_widget_height_rows)
                .min(area.height.saturating_sub(2))
        } else {
            1
        };
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1),
                Constraint::Min(0),
                Constraint::Length(widget_h),
            ])
            .split(area);
        self.draw_top_bar(frame, chunks[0]);
        self.draw_workspace(frame, chunks[1], &mut outcome);
        self.draw_context_widget(frame, chunks[2]);
        if self.log_overlay_visible {
            let lines = self.rt.log_buffer.snapshot();
            frame.render_widget(LogOverlay { lines }, area);
        }
        if self.keymap_overlay_visible {
            let entries = self.active_keymap_entries();
            frame.render_widget(KeymapOverlay { entries }, area);
        }
        if self.confirm_modal.open {
            let view = ConfirmModalView {
                state: &self.confirm_modal,
            };
            frame.render_widget(view, area);
        }
        if self.app_popover.is_some() {
            self.draw_app_popover(area, frame.buffer_mut());
        }
        outcome
    }

    /// Paint the Context Growth Widget strip (or its collapsed bar) at the
    /// bottom of the workspace.
    fn draw_context_widget(&mut self, frame: &mut ratatui::Frame<'_>, area: Rect) {
        if area.height == 0 {
            return;
        }
        if !self.workspace.context_widget_visible {
            ContextGrowthWidget::render_collapsed(area, frame.buffer_mut());
            return;
        }
        let session = self.widget_session();
        let merged = self
            .widget_merged_context()
            .map(|(m, _, _)| m)
            .unwrap_or(MergedRows {
                rows: Vec::new(),
                max_current_tokens: 0,
            });
        let hovered = self.hovered_chat_pk.read().ok().and_then(|g| *g);
        let widget = ContextGrowthWidget {
            data: &merged,
            session_id: session.as_deref(),
            hovered_chat_pk: hovered,
        };
        widget.render(area, frame.buffer_mut(), &self.context_widget);
    }

    fn draw_top_bar(&mut self, frame: &mut ratatui::Frame<'_>, area: Rect) {
        // status dot in column 0, then text title + hints.
        if area.width < 4 {
            return;
        }
        let dot_area = Rect::new(area.x, area.y, 1, 1);
        let status = StatusDot::new(self.status);
        let title = status.title().to_string();
        frame.render_widget(status, dot_area);

        let hints = format!(
            " ghcp-mon attach │ {title} │ a:add │ x:rm │ Shift+←/→:move │ Tab:focus │ M:mouse({mouse}) │ ?:logs │ q:quit",
            mouse = if self.mouse_enabled { "on" } else { "off" },
        );
        let p = Paragraph::new(Span::styled(hints, Style::default().fg(Color::White)));
        let rest = Rect::new(area.x + 2, area.y, area.width - 2, 1);
        frame.render_widget(p, rest);
    }

    fn draw_workspace(
        &mut self,
        frame: &mut ratatui::Frame<'_>,
        area: Rect,
        outcome: &mut DrawOutcome,
    ) {
        if self.workspace.columns.is_empty() {
            let msg = Paragraph::new(Line::from(Span::styled(
                "no columns. add one from the top bar.",
                Style::default()
                    .fg(Color::DarkGray)
                    .add_modifier(Modifier::ITALIC),
            )))
            .block(Block::default().borders(Borders::ALL));
            frame.render_widget(msg, area);
            return;
        }

        // Layout columns by weight, respecting MIN_COL.
        let total_weight: f32 = self.workspace.columns.iter().map(|c| c.width).sum();
        let weights: Vec<u16> = self
            .workspace
            .columns
            .iter()
            .map(|c| ((c.width / total_weight) * area.width as f32) as u16)
            .collect();
        let constraints: Vec<Constraint> = weights
            .iter()
            .map(|w| Constraint::Length((*w).max(MIN_COL.min(area.width))))
            .collect();
        let cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(constraints)
            .split(area);

        // Per-column dispatch — every scenario type goes through the
        // trait now. The `&self.workspace.columns[i].config` borrow is
        // disjoint from the `&mut self.scenarios` / `&mut self.span_detail_memo`
        // borrows held by Ctx, so we can pass them into one call without
        // cloning column state.
        for i in 0..self.workspace.columns.len() {
            let rect = cols[i];
            let focused = self.focus.column_idx() == Some(i);
            let (col_id, title) = {
                let c = &self.workspace.columns[i];
                (c.id.clone(), c.title.clone())
            };

            let mut block = Block::default()
                .borders(Borders::ALL)
                .title(format!(" {title} "));
            if focused {
                block = block.border_style(Style::default().fg(Color::Cyan));
            }
            let inner = block.inner(rect);
            frame.render_widget(block, rect);
            if inner.width < 3 {
                let buf: &mut Buffer = frame.buffer_mut();
                let span = Span::styled("…", Style::default().fg(Color::DarkGray));
                buf.set_span(inner.x, inner.y, &span, inner.width);
                continue;
            }
            let buf: &mut Buffer = frame.buffer_mut();
            if let Some(scenario) = self.scenarios.get_mut(&col_id) {
                let cfg = &self.workspace.columns[i].config;
                let mut ctx = crate::tui::scenarios::Ctx::new(
                    &self.rt.api,
                    &self.rt.cache,
                    &self.workspace,
                    &self.hovered_chat_pk,
                    &mut self.span_detail_memo,
                );
                scenario.draw(&mut ctx, i, &col_id, cfg, inner, buf, focused, outcome);
            } else {
                let cfg = &self.workspace.columns[i].config;
                let st = self.workspace.columns[i].scenario_type;
                render_placeholder(inner, buf, st, cfg);
            }
        }
    }

    /// Read-or-fetch `["session-span-tree", cid]` keyed by cid directly.
    /// Returns the complete server tree (per the cache contract — DELTA's
    /// prior-chat-span walk MUST read this, not Spans' reveal-filtered view).
    fn cached_session_span_tree_by_cid(
        &self,
        cid: &str,
    ) -> Vec<crate::tui::model::SpanNode> {
        let api = self.rt.api.clone();
        let cid_s = cid.to_string();
        swr_read::<SessionSpanTreeResponse, _, _>(
            &self.rt.cache,
            qkey(["session-span-tree", cid]),
            std::time::Duration::from_secs(5),
            FetchPolicy::Swr,
            move || async move {
                let r = api.get_session_span_tree(&cid_s).await?;
                Ok::<Value, anyhow::Error>(serde_json::to_value(r)?)
            },
        )
        .map(|r| r.tree)
        .unwrap_or_default()
    }

    fn scenario_type_options() -> Vec<String> {
        ScenarioType::all()
            .iter()
            .map(|st| st.default_title().to_string())
            .collect()
    }

    /// App-level popover overlay for global workspace actions.
    fn draw_app_popover(&mut self, area: Rect, buf: &mut Buffer) {
        let Some(popover) = self.app_popover else {
            return;
        };
        match popover {
            AppPopover::AddColumn => {
                let options = Self::scenario_type_options();
                SelectPopover {
                    title: "Add column",
                    options: &options,
                    cursor: self.add_column_picker.cursor,
                }
                .render(area, buf);
            }
        }
    }

}

/// Run the event loop until quit. Caller is responsible for `ratatui::init`
/// and the panic-hook guard.
///
/// Architecture: one `tokio::select!` over four event sources plus an
/// optional animation deadline. The loop owns `ws_rx`, `status_rx`, the
/// crossterm `EventStream`, and an owned `Arc<Notify>` cloned from the
/// query cache (background fetch completions ping it). After the first
/// branch resolves, any extra messages that arrived during processing are
/// drained via `try_recv` so a burst of envelopes still produces a single
/// `terminal.draw`. A fully idle TUI parks indefinitely.
pub async fn event_loop(
    terminal: &mut DefaultTerminal,
    app: &mut App,
) -> Result<()> {
    let mut ws_rx = app.rt.ws.subscribe();
    let mut status_rx = app.rt.ws.on_status();
    let mut keys = EventStream::new();
    let cache_changed = app.rt.cache.changed_handle();
    let mut ws_batch: Vec<WsEnvelope> = Vec::with_capacity(16);
    let mut last_outcome = DrawOutcome::default();

    // Paint once before parking; capture the renderer's outcome so the
    // next animation deadline reflects what was actually drawn.
    terminal.draw(|f| {
        last_outcome = app.draw(f);
    })?;

    loop {
        let mut dirty = false;
        let mut quit = false;

        tokio::select! {
            biased;

            _ = sleep_until_anim(
                app.next_anim_deadline_ms(last_outcome.spinner_visible),
                app.now_ms(),
            ) => {
                app.tick_anim();
                dirty = true;
            }

            ev = ws_rx.recv() => match ev {
                Ok(env) => { ws_batch.push(env); }
                Err(RecvError::Lagged(n)) => {
                    warn!(lagged = n, "ws receiver lagged");
                }
                Err(RecvError::Closed) => break,
            },

            st = status_rx.recv() => match st {
                Ok(s) => { app.status = s; dirty = true; }
                Err(RecvError::Lagged(_)) => {
                    // Re-sync from the bus on lag.
                    let s = app.rt.ws.status();
                    if app.status != s { app.status = s; dirty = true; }
                }
                Err(RecvError::Closed) => {}
            },

            key_ev = keys.next() => match key_ev {
                Some(Ok(crossterm::event::Event::Key(k)))
                    if k.kind == KeyEventKind::Press =>
                {
                    quit = app.handle_key(k)?;
                    dirty = true;
                }
                Some(Ok(crossterm::event::Event::Resize(w, h))) => {
                    app.term_size = (w, h);
                    dirty = true;
                }
                Some(Ok(_)) => {}
                Some(Err(e)) => {
                    warn!(error = %e, "crossterm event stream error");
                    break;
                }
                None => break,
            },

            _ = cache_changed.notified() => {
                // A background fetch populated (or invalidated) a cache
                // entry — re-run on_cache_changed on every scenario so
                // follow-mode columns can catch up to the freshly-arrived
                // span tree. Idempotent for unchanged trees.
                app.dispatch_cache_changed();
                dirty = true;
            }
        }

        if quit {
            break;
        }

        // Drain any other WS envelopes that arrived during processing so
        // bursts collapse into a single batched invalidation + draw.
        loop {
            match ws_rx.try_recv() {
                Ok(env) => { ws_batch.push(env); }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Lagged(n)) => {
                    warn!(lagged = n, "ws receiver lagged during drain");
                }
                Err(TryRecvError::Closed) => break,
            }
        }
        if !ws_batch.is_empty() {
            app.on_ws_envelopes(ws_batch.drain(..));
            dirty = true;
        }
        loop {
            match status_rx.try_recv() {
                Ok(s) => { app.status = s; dirty = true; }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Lagged(_)) => {
                    let s = app.rt.ws.status();
                    if app.status != s { app.status = s; dirty = true; }
                }
                Err(TryRecvError::Closed) => break,
            }
        }

        if dirty {
            terminal.draw(|f| {
                last_outcome = app.draw(f);
            })?;
        }
    }
    Ok(())
}

/// Sleep until `at_ms` wall-clock millisecond, or park forever if `None`.
/// `now_ms` is sampled by the caller to avoid two `SystemTime::now()` calls
/// per iteration.
async fn sleep_until_anim(at_ms: Option<u64>, now_ms: u64) {
    match at_ms {
        Some(target) => {
            let delay = target.saturating_sub(now_ms);
            tokio::time::sleep(std::time::Duration::from_millis(delay)).await;
        }
        None => std::future::pending::<()>().await,
    }
}

/// Make App::draw render into a buffer for unit-testing (no full terminal).
#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::ws::WsBus;

    fn make_app() -> App {
        let api = ApiClient::new("http://127.0.0.1:4319".into());
        let ws = WsBus::new("ws://127.0.0.1:4319/ws/events".into());
        let mut app = App::new(api, ws, LogBuffer::new(), false);
        // The Context Growth Widget defaults to visible in production; hide it
        // in the shared test harness so workspace-rendering tests are not
        // squeezed by the bottom strip. Widget tests re-enable it explicitly.
        app.workspace.context_widget_visible = false;
        app
    }

    #[test]
    fn append_column_focuses_appended_column() {
        let mut app = make_app();
        app.workspace.columns.clear();
        app.focus = Focus::None;
        app.last_focused_column = None;

        app.append_column(ScenarioType::LiveSessions);

        assert_eq!(app.workspace.columns.len(), 1);
        assert_eq!(app.workspace.columns[0].scenario_type, ScenarioType::LiveSessions);
        assert_eq!(app.focus.column_idx(), Some(0));
    }

    /// Regression for bug "FileTouches opens empty until session is
    /// re-selected": columns added AFTER session propagation already ran
    /// must inherit the active `session` from any sibling column whose
    /// type also participates in session propagation.
    #[test]
    fn append_column_inherits_active_session_and_selection() {
        let mut app = make_app();
        app.workspace.columns.clear();
        app.focus = Focus::None;
        app.last_focused_column = None;

        // Simulate "session already picked": an existing Spans column
        // carries the active session + selection + search.
        app.append_column(ScenarioType::Spans);
        let spans_idx = 0;
        app.workspace.columns[spans_idx]
            .config
            .insert("session".into(), toml::Value::String("cid-1".into()));
        app.workspace.columns[spans_idx]
            .config
            .insert("selected_trace_id".into(), toml::Value::String("trace-x".into()));
        app.workspace.columns[spans_idx]
            .config
            .insert("selected_span_id".into(), toml::Value::String("span-x".into()));
        app.workspace.columns[spans_idx]
            .config
            .insert("search_query".into(), toml::Value::String("foo".into()));

        // Append a FileTouches column. It must inherit `session`.
        app.append_column(ScenarioType::FileTouches);
        let ft = &app.workspace.columns[1];
        assert_eq!(ft.scenario_type, ScenarioType::FileTouches);
        assert_eq!(ft.config.get("session").and_then(|v| v.as_str()), Some("cid-1"));

        // Append a ChatDetail column. It must inherit `session`,
        // `selected_*`, and `search_query`.
        app.append_column(ScenarioType::ChatDetail);
        let cd = &app.workspace.columns[2];
        assert_eq!(cd.scenario_type, ScenarioType::ChatDetail);
        assert_eq!(cd.config.get("session").and_then(|v| v.as_str()), Some("cid-1"));
        assert_eq!(
            cd.config.get("selected_trace_id").and_then(|v| v.as_str()),
            Some("trace-x")
        );
        assert_eq!(
            cd.config.get("selected_span_id").and_then(|v| v.as_str()),
            Some("span-x")
        );
        assert_eq!(cd.config.get("search_query").and_then(|v| v.as_str()), Some("foo"));

        // Append a ToolDetail column. Inherits `selected_*` + `search_query`
        // but NOT `session` (not in its propagation set).
        app.append_column(ScenarioType::ToolDetail);
        let td = &app.workspace.columns[3];
        assert_eq!(td.scenario_type, ScenarioType::ToolDetail);
        assert!(td.config.get("session").is_none(), "ToolDetail must not inherit session");
        assert_eq!(
            td.config.get("selected_trace_id").and_then(|v| v.as_str()),
            Some("trace-x")
        );
        assert_eq!(td.config.get("search_query").and_then(|v| v.as_str()), Some("foo"));

        // Append a LiveSessions column. Inherits nothing — not in any
        // propagation set.
        app.append_column(ScenarioType::LiveSessions);
        let ls = &app.workspace.columns[4];
        assert_eq!(ls.scenario_type, ScenarioType::LiveSessions);
        assert!(ls.config.get("session").is_none());
        assert!(ls.config.get("selected_trace_id").is_none());
        assert!(ls.config.get("search_query").is_none());
    }

    #[test]
    fn pressing_a_opens_add_column_popover_and_enter_adds_selected_scenario() {
        let mut app = make_app();
        app.workspace.columns.clear();
        app.focus = Focus::None;
        app.last_focused_column = None;

        let _ = app.handle_key(press(KeyCode::Char('a'))).unwrap();
        assert_eq!(app.app_popover, Some(AppPopover::AddColumn));
        assert!(app.add_column_picker.open);
        let text = render_buf_text(&mut app, 80, 12);
        assert!(text.contains("Add column"), "popover not rendered:\n{text}");

        let _ = app.handle_key(press(KeyCode::Down)).unwrap();
        let _ = app.handle_key(press(KeyCode::Enter)).unwrap();

        assert_eq!(app.workspace.columns.len(), 1);
        assert_eq!(app.workspace.columns[0].scenario_type, ScenarioType::Spans);
        assert_eq!(app.focus.column_idx(), Some(0));
        assert_eq!(app.app_popover, None);
        assert!(!app.add_column_picker.open);
    }

    #[test]
    fn shift_arrows_move_focused_column_and_focus_follows() {
        let mut app = make_app();
        app.workspace = Workspace::seeded_default();
        app.workspace.context_widget_visible = false;
        app.focus_column(2);

        let _ = app
            .handle_key(key_mod(KeyCode::Left, KeyModifiers::SHIFT))
            .unwrap();
        assert_eq!(app.workspace.columns[1].scenario_type, ScenarioType::ToolDetail);
        assert_eq!(app.focus.column_idx(), Some(1));

        let _ = app
            .handle_key(key_mod(KeyCode::Left, KeyModifiers::SHIFT))
            .unwrap();
        assert_eq!(app.workspace.columns[0].scenario_type, ScenarioType::ToolDetail);
        assert_eq!(app.focus.column_idx(), Some(0));

        let _ = app
            .handle_key(key_mod(KeyCode::Right, KeyModifiers::SHIFT))
            .unwrap();
        assert_eq!(app.workspace.columns[1].scenario_type, ScenarioType::ToolDetail);
        assert_eq!(app.focus.column_idx(), Some(1));
    }

    #[test]
    fn empty_workspace_renders_hint() {
        use ratatui::Terminal;
        use ratatui::backend::TestBackend;
        let mut app = make_app();
        app.workspace.columns.clear();
        app.focus = Focus::None;
        let backend = TestBackend::new(80, 12);
        let mut term = Terminal::new(backend).unwrap();
        term.draw(|f| {
            app.draw(f);
        })
        .unwrap();
        let buf = term.backend().buffer();
        let mut joined = String::new();
        for y in 0..buf.area.height {
            for x in 0..buf.area.width {
                joined.push_str(buf[(x, y)].symbol());
            }
            joined.push('\n');
        }
        assert!(
            joined.contains("no columns. add one from the top bar."),
            "buffer was:\n{joined}"
        );
    }

    /// Many WS envelopes in a row must remain side-effect-clean (no panics,
    /// idempotent cache invalidation).
    #[tokio::test(flavor = "current_thread")]
    async fn handle_processes_many_ws_envelopes() {
        use serde_json::json;
        let mut app = make_app();
        // Batch form (production path): one call collapses N envelopes
        // into one cache scan per dirtied prefix.
        let envs = (0..50).map(|_| WsEnvelope {
            kind: WsKind::Span,
            entity: crate::tui::model::WsEntity::Span,
            payload: json!({}),
        });
        app.on_ws_envelopes(envs);
        // Single-envelope convenience form must also remain idempotent.
        for _ in 0..10 {
            app.on_ws_envelope(WsEnvelope {
                kind: WsKind::Metric,
                entity: crate::tui::model::WsEntity::Metric,
                payload: json!({}),
            });
        }
    }

    /// Allow Duration unused-warning suppression: tests may evolve.
    #[allow(dead_code)]
    fn _unused() -> std::time::Duration {
        std::time::Duration::from_millis(1)
    }

    // ---- Phase 1 wiring tests ----

    use crate::tui::cache::FetchedRecord;
    use crate::tui::model::*;
    use crate::tui::scenarios::spans::SpansPopover;

    fn mk_span_node(
        id: &str,
        kind: KindClass,
        tool_name: Option<&str>,
        end_ns: i128,
        children: Vec<SpanNode>,
    ) -> SpanNode {
        mk_span_node_named(id, id, kind, tool_name, end_ns, children)
    }

    fn mk_span_node_named(
        id: &str,
        name: &str,
        kind: KindClass,
        tool_name: Option<&str>,
        end_ns: i128,
        children: Vec<SpanNode>,
    ) -> SpanNode {
        let projection = SpanProjection {
            tool_call: tool_name.map(|n| ToolCallProjection {
                tool_call_pk: 0,
                call_id: Some(format!("call-{id}")),
                tool_name: Some(n.to_string()),
                ..Default::default()
            }),
            ..Default::default()
        };
        SpanNode {
            span_pk: end_ns as i64,
            trace_id: "trace-1".into(),
            span_id: id.into(),
            parent_span_id: None,
            name: name.into(),
            kind_class: kind,
            ingestion_state: "complete".into(),
            start_unix_ns: Some(end_ns),
            end_unix_ns: Some(end_ns),
            projection,
            children,
        }
    }

    fn mk_external_tool_node(id: &str, name: &str, tool_name: &str, end_ns: i128) -> SpanNode {
        let mut node = mk_span_node_named(id, name, KindClass::ExternalTool, None, end_ns, vec![]);
        node.projection.external_tool_call = Some(ExternalToolCallProjection {
            ext_pk: 1,
            call_id: Some(format!("ext-{id}")),
            tool_name: Some(tool_name.to_string()),
            paired_tool_call_pk: None,
            conversation_id: None,
            agent_run_pk: None,
        });
        node
    }

    fn seed_session_tree(app: &App, cid: &str, tree: Vec<SpanNode>) {
        let resp = SessionSpanTreeResponse {
            conversation_id: cid.into(),
            tree,
        };
        // Match-or-exceed the cache's current latest_gen so `put` accepts
        // the seed even after a prior `invalidate` bumped the generation.
        // The helper is test-only; production seeds come from real
        // fetcher completions whose generation always matches.
        let key = crate::tui::cache::qkey(["session-span-tree", cid]);
        let cur_gen = app.rt.cache.peek(&key).value.as_ref().map(|c| c.generation).unwrap_or(0);
        app.rt.cache.put(FetchedRecord {
            key,
            generation: cur_gen.saturating_add(1),
            value: serde_json::to_value(resp).unwrap(),
            stale_after: std::time::Duration::from_secs(60),
        });
    }

    fn seed_span_detail(
        app: &App,
        trace_id: &str,
        span_id: &str,
        attrs: serde_json::Value,
    ) {
        let span = SpanFull {
            span_pk: 1,
            trace_id: trace_id.into(),
            span_id: span_id.into(),
            parent_span_id: None,
            name: span_id.into(),
            kind: Some(1),
            kind_class: KindClass::ExecuteTool,
            start_unix_ns: Some(100),
            end_unix_ns: Some(200),
            duration_ns: Some(100),
            status_message: None,
            ingestion_state: "complete".into(),
            scope_name: None,
            scope_version: None,
            attributes: Some(attrs),
            resource: None,
        };
        let detail = SpanDetail {
            span,
            events: vec![],
            parent: None,
            children: vec![],
            projection: SpanProjection::default(),
        };
        app.rt.cache.put(FetchedRecord {
            key: crate::tui::cache::qkey(["span", trace_id, span_id]),
            generation: 1,
            value: serde_json::to_value(detail).unwrap(),
            stale_after: std::time::Duration::from_secs(60),
        });
    }

    fn seed_sessions(app: &App, sessions: Vec<SessionSummary>) {
        let r = ListSessionsResponse { sessions };
        app.rt.cache.put(FetchedRecord {
            key: crate::tui::cache::qkey(["sessions"]),
            generation: 1,
            value: serde_json::to_value(r).unwrap(),
            stale_after: std::time::Duration::from_secs(60),
        });
    }

    fn seed_search(app: &App, session: &str, q: &str, hits: Vec<&str>) {
        let resp = SearchResponse {
            results: hits
                .iter()
                .map(|sid| SearchSpanResult {
                    span_pk: 0,
                    trace_id: "trace-1".into(),
                    span_id: (*sid).into(),
                    parent_span_id: None,
                    name: (*sid).into(),
                    kind_class: KindClass::ExecuteTool,
                    start_unix_ns: None,
                    end_unix_ns: None,
                    ingestion_state: "complete".into(),
                    projection: SpanProjection::default(),
                    matches: vec![],
                })
                .collect(),
        };
        app.rt.cache.put(FetchedRecord {
            key: crate::tui::cache::qkey(["search-spans", session, q]),
            generation: 1,
            value: serde_json::to_value(resp).unwrap(),
            stale_after: std::time::Duration::from_secs(60),
        });
    }

    fn seed_traces(app: &App, traces: Vec<TraceSummary>) {
        let r = ListTracesResponse { traces };
        app.rt.cache.put(FetchedRecord {
            key: crate::tui::cache::qkey(["traces"]),
            generation: 1,
            value: serde_json::to_value(r).unwrap(),
            stale_after: std::time::Duration::from_secs(60),
        });
    }

    fn render_buf(app: &mut App, w: u16, h: u16) -> Buffer {
        use ratatui::Terminal;
        use ratatui::backend::TestBackend;
        let backend = TestBackend::new(w, h);
        let mut term = Terminal::new(backend).unwrap();
        term.draw(|f| {
            app.draw(f);
        })
        .unwrap();
        term.backend().buffer().clone()
    }

    fn buf_text(buf: &Buffer) -> String {
        let mut joined = String::new();
        for y in 0..buf.area.height {
            for x in 0..buf.area.width {
                joined.push_str(buf[(x, y)].symbol());
            }
            joined.push('\n');
        }
        joined
    }

    fn render_buf_text(app: &mut App, w: u16, h: u16) -> String {
        let buf = render_buf(app, w, h);
        buf_text(&buf)
    }

    fn row_text(buf: &Buffer, y: u16) -> String {
        let mut row = String::new();
        for x in 0..buf.area.width {
            row.push_str(buf[(x, y)].symbol());
        }
        row
    }

    fn row_substr_x(buf: &Buffer, y: u16, needle: &str) -> Option<u16> {
        // The buffer's columns are CELLS; symbols can be multi-byte
        // (the chip outlines `▏`/`▕` are 3 bytes each). `String::find`
        // returns a *byte* offset into the joined row text — cast to
        // a cell index it would land 2 cells past the start of any
        // multi-byte symbol preceding the needle. Walk cell-by-cell
        // accumulating symbol bytes and report the cell whose byte
        // offset matches a `String::find` hit.
        let row = row_text(buf, y);
        let byte_off = row.find(needle)?;
        let mut bytes = 0usize;
        for x in 0..buf.area.width {
            if bytes == byte_off {
                return Some(x);
            }
            if bytes > byte_off {
                return None;
            }
            bytes += buf[(x, y)].symbol().len();
        }
        None
    }

    fn one_spans_column_app() -> App {
        let mut app = make_app();
        app.workspace.columns.clear();
        app.workspace
            .add_column(crate::tui::workspace::ScenarioType::Spans);
        app.sync_scenarios_with_workspace();
        app.focus = Focus::Column(0);
        app.last_focused_column = Some(0);
        app
    }

    fn tool_detail_then_spans_app() -> App {
        let mut app = make_app();
        app.workspace.columns.clear();
        app.workspace
            .add_column(crate::tui::workspace::ScenarioType::ToolDetail);
        app.workspace
            .add_column(crate::tui::workspace::ScenarioType::Spans);
        app.sync_scenarios_with_workspace();
        app.focus = Focus::Column(0);
        app.last_focused_column = Some(0);
        app
    }

    #[tokio::test(flavor = "current_thread")]
    async fn chips_render_execute_tool_name_as_hash_chip_without_kind_or_model() {
        let mut app = one_spans_column_app();
        app.workspace.columns[0]
            .config
            .insert("session".into(), toml::Value::String("cid-1".into()));
        let tree = vec![mk_span_node_named(
            "bash-span",
            "gpt-4 - bash",
            KindClass::ExecuteTool,
            Some("bash"),
            100,
            vec![],
        )];
        seed_session_tree(&app, "cid-1", tree);

        let buf = render_buf(&mut app, 120, 20);
        let text = buf_text(&buf);
        let row = row_text(&buf, 4);
        assert!(row.contains("bash"), "missing bash tool chip in:\n{text}");
        assert!(!row.contains("gpt-4"), "model leaked into row:\n{text}");
        assert!(!row.contains("tool"), "generic tool kind badge leaked into row:\n{text}");
        let x = row_substr_x(&buf, 4, "bash").expect("bash x");
        assert_eq!(
            buf[(x, 4)].style().fg,
            Some(crate::tui::format::hash_color("bash"))
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn chips_render_external_tool_name_as_hash_chip_without_kind_or_model() {
        let mut app = one_spans_column_app();
        app.workspace.columns[0]
            .config
            .insert("session".into(), toml::Value::String("cid-1".into()));
        let tree = vec![mk_external_tool_node(
            "external-span",
            "claude-3 - web_fetch",
            "web_fetch",
            100,
        )];
        seed_session_tree(&app, "cid-1", tree);

        let buf = render_buf(&mut app, 120, 20);
        let text = buf_text(&buf);
        let row = row_text(&buf, 4);
        assert!(row.contains("web_fetch"), "missing external tool chip in:\n{text}");
        assert!(!row.contains("claude-3"), "model leaked into row:\n{text}");
        assert!(!row.contains("external"), "generic external kind badge leaked into row:\n{text}");
        let x = row_substr_x(&buf, 4, "web_fetch").expect("web_fetch x");
        assert_eq!(
            buf[(x, 4)].style().fg,
            Some(crate::tui::format::hash_color("web_fetch"))
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn chips_render_target_path_in_tree_row() {
        let mut app = one_spans_column_app();
        app.workspace.columns[0]
            .config
            .insert("session".into(), toml::Value::String("cid-1".into()));
        let tree = vec![mk_span_node(
            "edit-span",
            KindClass::ExecuteTool,
            Some("edit"),
            100,
            vec![],
        )];
        seed_session_tree(&app, "cid-1", tree);
        seed_span_detail(
            &app,
            "trace-1",
            "edit-span",
            serde_json::json!({
                "gen_ai.tool.call.arguments": {
                    "path": "/repo/src/lib.rs",
                    "old_str": "",
                    "new_str": ""
                }
            }),
        );

        let buf = render_buf(&mut app, 120, 20);
        let text = buf_text(&buf);
        let row = row_text(&buf, 4);
        assert!(row.contains("lib.rs"), "missing target path chip in:\n{text}");
        let x = row_substr_x(&buf, 4, "lib.rs").expect("lib.rs x");
        assert_eq!(buf[(x, 4)].style().fg, Some(Color::Cyan));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn chips_render_target_url_hostname_in_tree_row() {
        let mut app = one_spans_column_app();
        app.workspace.columns[0]
            .config
            .insert("session".into(), toml::Value::String("cid-1".into()));
        let tree = vec![mk_span_node(
            "fetch-span",
            KindClass::ExecuteTool,
            Some("web_fetch"),
            100,
            vec![],
        )];
        seed_session_tree(&app, "cid-1", tree);
        seed_span_detail(
            &app,
            "trace-1",
            "fetch-span",
            serde_json::json!({
                "gen_ai.tool.call.arguments": {
                    "url": "https://example.com/docs/page"
                }
            }),
        );

        let buf = render_buf(&mut app, 120, 20);
        let text = buf_text(&buf);
        let row = row_text(&buf, 4);
        assert!(row.contains("example.com"), "missing target URL chip in:\n{text}");
        let x = row_substr_x(&buf, 4, "example.com").expect("example.com x");
        assert_eq!(buf[(x, 4)].style().fg, Some(Color::Cyan));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn chips_render_diff_stat_in_tree_row() {
        let mut app = one_spans_column_app();
        app.workspace.columns[0]
            .config
            .insert("session".into(), toml::Value::String("cid-1".into()));
        let tree = vec![mk_span_node(
            "edit-span",
            KindClass::ExecuteTool,
            Some("edit"),
            100,
            vec![],
        )];
        seed_session_tree(&app, "cid-1", tree);
        // Seed span detail with edit arguments.
        seed_span_detail(
            &app,
            "trace-1",
            "edit-span",
            serde_json::json!({
                "gen_ai.tool.call.arguments": {
                    "old_str": "a\nb\nc",
                    "new_str": "x"
                }
            }),
        );
        let text = render_buf_text(&mut app, 120, 20);
        // -3 (removed) and +1 (added) chips must appear somewhere.
        assert!(text.contains("-3"), "missing -3 in:\n{text}");
        assert!(text.contains("+1"), "missing +1 in:\n{text}");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn chips_render_skill_and_description_in_tree_row() {
        let mut app = one_spans_column_app();
        app.workspace.columns[0]
            .config
            .insert("session".into(), toml::Value::String("cid-1".into()));
        let tree = vec![mk_span_node(
            "skill-span",
            KindClass::ExecuteTool,
            Some("skill"),
            100,
            vec![],
        )];
        seed_session_tree(&app, "cid-1", tree);
        seed_span_detail(
            &app,
            "trace-1",
            "skill-span",
            serde_json::json!({
                "gen_ai.tool.call.arguments": {
                    "skill": "summarize",
                    "description": "skim docs"
                }
            }),
        );
        let text = render_buf_text(&mut app, 120, 20);
        assert!(text.contains("summarize"), "missing skill chip in:\n{text}");
        assert!(text.contains("skim docs"), "missing desc in:\n{text}");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn chips_render_report_intent_title_on_parent_row() {
        let mut app = one_spans_column_app();
        app.workspace.columns[0]
            .config
            .insert("session".into(), toml::Value::String("cid-1".into()));
        let parent = mk_span_node(
            "parent",
            KindClass::InvokeAgent,
            None,
            100,
            vec![mk_span_node(
                "report-child",
                KindClass::ExecuteTool,
                Some("report_intent"),
                150,
                vec![],
            )],
        );
        seed_session_tree(&app, "cid-1", vec![parent]);
        seed_span_detail(
            &app,
            "trace-1",
            "report-child",
            serde_json::json!({
                "gen_ai.tool.call.arguments": {"intent": "DOTHETHING"}
            }),
        );
        let text = render_buf_text(&mut app, 120, 20);
        assert!(text.contains("DOTHETHING"), "missing intent in:\n{text}");
    }

    /// Regression for bug "chat lines show redundant `[chat] chat`":
    /// the kind badge already says `chat`; the row name is empty for
    /// chat-kind spans. The freed space hosts the first ~20 chars of
    /// the chat's captured text (assistant reply preferred, then most
    /// recent input message).
    #[tokio::test(flavor = "current_thread")]
    async fn chat_row_suppresses_chat_name_and_shows_text_preview() {
        let mut app = one_spans_column_app();
        app.workspace.columns[0]
            .config
            .insert("session".into(), toml::Value::String("cid-1".into()));
        let tree = vec![mk_span_node(
            "chat-span",
            KindClass::Chat,
            None,
            100,
            vec![],
        )];
        seed_session_tree(&app, "cid-1", tree);

        // Manually seed the chat span detail (existing seed_span_detail
        // hardcodes kind_class=ExecuteTool, which would mis-flag the
        // node). We use the same FetchedRecord shape it uses.
        let span = crate::tui::model::SpanFull {
            span_pk: 1,
            trace_id: "trace-1".into(),
            span_id: "chat-span".into(),
            parent_span_id: None,
            name: "chat".into(),
            kind: Some(1),
            kind_class: KindClass::Chat,
            start_unix_ns: Some(100),
            end_unix_ns: Some(200),
            duration_ns: Some(100),
            status_message: None,
            ingestion_state: "complete".into(),
            scope_name: None,
            scope_version: None,
            attributes: Some(serde_json::json!({
                "gen_ai.output.messages": [
                    {"role": "assistant", "parts": [
                        {"type": "text", "content": "Hello there friend"}
                    ]}
                ]
            })),
            resource: None,
        };
        let detail = SpanDetail {
            span,
            events: vec![],
            parent: None,
            children: vec![],
            projection: SpanProjection::default(),
        };
        app.rt.cache.put(FetchedRecord {
            key: crate::tui::cache::qkey(["span", "trace-1", "chat-span"]),
            generation: 1,
            value: serde_json::to_value(detail).unwrap(),
            stale_after: std::time::Duration::from_secs(60),
        });

        let text = render_buf_text(&mut app, 120, 20);
        // The kind badge prints "chat" exactly once on the chat row.
        // After the fix the row name no longer adds a second redundant
        // "chat" — i.e., we must NOT see "chat  chat" (badge + name)
        // anywhere on the rendered surface. We allow the badge itself
        // (`chat`) plus its trailing-space-padded ` chat ` form.
        assert!(
            !text.contains("chat  chat"),
            "redundant 'chat' name printed alongside [chat] badge:\n{text}"
        );
        // And the preview text shows up.
        assert!(
            text.contains("Hello there friend"),
            "chat preview missing in:\n{text}"
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn follow_mode_advances_cursor_to_latest_tool_span_on_ws_envelope() {
        use serde_json::json;
        let mut app = one_spans_column_app();
        app.workspace.columns[0]
            .config
            .insert("session".into(), toml::Value::String("cid-1".into()));
        let tree = vec![
            mk_span_node("chat-1", KindClass::Chat, None, 50, vec![]),
            mk_span_node("tool-a", KindClass::ExecuteTool, Some("bash"), 100, vec![]),
            mk_span_node("tool-b", KindClass::ExecuteTool, Some("bash"), 200, vec![]),
        ];
        seed_session_tree(&app, "cid-1", tree);
        // Engage follow mode manually.
        let col_id = app.workspace.columns[0].id.clone();
        app.spans_scenario_mut(&col_id).unwrap().state_mut()
            .follow_mode = true;
        // Drive a spans-touching WS envelope (replaces the old Tick drive).
        app.on_ws_envelope(WsEnvelope {
            kind: WsKind::Span,
            entity: crate::tui::model::WsEntity::Span,
            payload: json!({}),
        });
        // Latest tool span is "tool-b" (end_ns=200) at flat index 2.
        let st = app.spans_scenario(&col_id).unwrap().state();
        assert_eq!(st.cursor, 2);
        assert_eq!(st.focused_span_id.as_deref(), Some("tool-b"));
    }

    /// Regression for bug "follow mode picks the one before the latest":
    /// `on_ws_envelopes` invalidates the cache *and then* immediately
    /// dispatches `on_ws_batch`, which reads through `swr_read` and gets
    /// the still-stale tree (the new tool span hasn't been fetched
    /// yet). Cursor lands one span behind. The fix is for the
    /// `cache_changed` arm of the event loop to call
    /// `dispatch_cache_changed`, which re-runs `on_cache_changed` on
    /// every scenario — Spans then re-advances follow-mode against the
    /// freshly-arrived tree.
    #[tokio::test(flavor = "current_thread")]
    async fn follow_mode_advances_again_after_cache_changed() {
        use serde_json::json;
        let mut app = one_spans_column_app();
        app.workspace.columns[0]
            .config
            .insert("session".into(), toml::Value::String("cid-1".into()));
        // Initial tree: only tool-a exists.
        let tree_v1 = vec![
            mk_span_node("chat-1", KindClass::Chat, None, 50, vec![]),
            mk_span_node("tool-a", KindClass::ExecuteTool, Some("bash"), 100, vec![]),
        ];
        seed_session_tree(&app, "cid-1", tree_v1);

        let col_id = app.workspace.columns[0].id.clone();
        app.spans_scenario_mut(&col_id).unwrap().state_mut()
            .follow_mode = true;

        // Initial WS envelope advances cursor onto tool-a (the only tool).
        app.on_ws_envelope(WsEnvelope {
            kind: WsKind::Span,
            entity: crate::tui::model::WsEntity::Span,
            payload: json!({}),
        });
        assert_eq!(app.spans_scenario(&col_id).unwrap().state().focused_span_id.as_deref(), Some("tool-a"));

        // Production race: the cache invalidates and a fresh tree
        // containing tool-b lands later, after the WS envelope's
        // `on_ws_batch` dispatch already ran against the stale tree.
        // Simulate by replacing the cached value.
        app.rt.cache.invalidate(&["session-span-tree"]);
        let tree_v2 = vec![
            mk_span_node("chat-1", KindClass::Chat, None, 50, vec![]),
            mk_span_node("tool-a", KindClass::ExecuteTool, Some("bash"), 100, vec![]),
            mk_span_node("tool-b", KindClass::ExecuteTool, Some("bash"), 200, vec![]),
        ];
        seed_session_tree(&app, "cid-1", tree_v2);

        // Cache-changed wakeup. Must re-run follow-mode advance via
        // the `on_cache_changed` hook (production: `event_loop`'s
        // `cache_changed.notified()` arm calls `dispatch_cache_changed`).
        app.dispatch_cache_changed();
        let st = app.spans_scenario(&col_id).unwrap().state();
        assert_eq!(
            st.focused_span_id.as_deref(),
            Some("tool-b"),
            "follow-mode must catch up to the newest tool span once the cache lands it"
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn server_search_hit_highlights_matching_rows() {
        let mut app = one_spans_column_app();
        app.workspace.columns[0]
            .config
            .insert("session".into(), toml::Value::String("cid-1".into()));
        // Use InvokeAgent-kind nodes so the row name renders. ExecuteTool /
        // ExternalTool names are suppressed because chips carry the tool
        // identity; Chat names are also suppressed (the kind badge already
        // says "chat" and the description slot holds the text preview).
        let tree = vec![
            mk_span_node("hit-span", KindClass::InvokeAgent, None, 100, vec![]),
            mk_span_node("miss-span", KindClass::InvokeAgent, None, 200, vec![]),
        ];
        seed_session_tree(&app, "cid-1", tree);
        let col_id = app.workspace.columns[0].id.clone();
        let s = app.spans_scenario_mut(&col_id).unwrap().state_mut();
        s.search.set_text("hit");
        s.last_search_emitted = "hit".to_string();
        seed_search(&app, "cid-1", "hit", vec!["hit-span"]);
        // Render and assert the hit row name appears and the miss row is
        // present too (no hiding). Cell styling is hard to assert in plain
        // text — but the row content must be unchanged.
        let text = render_buf_text(&mut app, 120, 20);
        assert!(text.contains("hit-span"), "missing hit in:\n{text}");
        assert!(text.contains("miss-span"), "missing miss in:\n{text}");
    }

    /// LLR: `TUI Spans traces list mode` — "Arrow keys move the cursor;
    /// `Enter` is reserved for Phase 6 trace-pick." In no-session mode the
    /// session span tree is empty, so navigation MUST target `traces_cursor`
    /// against the cached traces list, not `cursor` against the empty tree.
    #[tokio::test(flavor = "current_thread")]
    async fn traces_mode_down_advances_traces_cursor() {
        let mut app = one_spans_column_app();
        seed_traces(
            &app,
            vec![
                TraceSummary {
                    trace_id: "trace-a".into(),
                    first_seen_ns: None,
                    last_seen_ns: None,
                    span_count: 1,
                    placeholder_count: 0,
                    kind_counts: KindCounts::default(),
                    root: None,
                    conversation_id: None,
                },
                TraceSummary {
                    trace_id: "trace-b".into(),
                    first_seen_ns: None,
                    last_seen_ns: None,
                    span_count: 1,
                    placeholder_count: 0,
                    kind_counts: KindCounts::default(),
                    root: None,
                    conversation_id: None,
                },
                TraceSummary {
                    trace_id: "trace-c".into(),
                    first_seen_ns: None,
                    last_seen_ns: None,
                    span_count: 1,
                    placeholder_count: 0,
                    kind_counts: KindCounts::default(),
                    root: None,
                    conversation_id: None,
                },
            ],
        );
        let col_id = app.workspace.columns[0].id.clone();
        // No session in config → traces mode. Initial cursor = 0.
        assert_eq!(
            app.spans_scenario(&col_id)
                .map(|s| s.state().traces_cursor)
                .unwrap_or(0),
            0
        );
        let _ = app.handle_key(press(KeyCode::Down)).unwrap();
        assert_eq!(app.spans_scenario(&col_id).unwrap().state().traces_cursor, 1);
        let _ = app.handle_key(press(KeyCode::Down)).unwrap();
        assert_eq!(app.spans_scenario(&col_id).unwrap().state().traces_cursor, 2);
        // Clamps at last row.
        let _ = app.handle_key(press(KeyCode::Down)).unwrap();
        assert_eq!(app.spans_scenario(&col_id).unwrap().state().traces_cursor, 2);
        // Up wraps no further than 0.
        let _ = app.handle_key(press(KeyCode::Up)).unwrap();
        let _ = app.handle_key(press(KeyCode::Up)).unwrap();
        let _ = app.handle_key(press(KeyCode::Up)).unwrap();
        let _ = app.handle_key(press(KeyCode::Up)).unwrap();
        assert_eq!(app.spans_scenario(&col_id).unwrap().state().traces_cursor, 0);
    }

    /// LLR: `TUI Spans traces list mode` — "`Enter` is reserved for Phase 6
    /// trace-pick." It MUST be consumed (no-op) so it does not fall through
    /// to the global layer.
    #[tokio::test(flavor = "current_thread")]
    async fn traces_mode_enter_is_consumed_no_op() {
        let mut app = one_spans_column_app();
        seed_traces(
            &app,
            vec![TraceSummary {
                trace_id: "trace-a".into(),
                first_seen_ns: None,
                last_seen_ns: None,
                span_count: 1,
                placeholder_count: 0,
                kind_counts: KindCounts::default(),
                root: None,
                conversation_id: None,
            }],
        );
        // No selection should be propagated; no session should be set.
        let quit = app.handle_key(press(KeyCode::Enter)).unwrap();
        assert!(!quit);
        // Session config remains unset (no spans_pick fired).
        assert!(app.workspace.columns[0].config.get("session").is_none());
    }

    #[tokio::test(flavor = "current_thread")]
    async fn traces_mode_renders_when_no_session() {
        let mut app = one_spans_column_app();
        // No session in config.
        seed_traces(
            &app,
            vec![TraceSummary {
                trace_id: "abcdef0123456789".into(),
                first_seen_ns: Some(0),
                last_seen_ns: Some(0),
                span_count: 7,
                placeholder_count: 0,
                kind_counts: KindCounts {
                    chat: 1,
                    execute_tool: 5,
                    external_tool: 0,
                    invoke_agent: 1,
                    other: 0,
                },
                root: None,
                conversation_id: None,
            }],
        );
        let text = render_buf_text(&mut app, 120, 20);
        assert!(text.contains("abcdef01"), "missing trace id in:\n{text}");
        assert!(text.contains("spans:7"), "missing span count in:\n{text}");
        assert!(text.contains("chat:1"), "missing chat count in:\n{text}");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn span_detail_pane_shows_focused_row_summary() {
        let mut app = one_spans_column_app();
        app.workspace.columns[0]
            .config
            .insert("session".into(), toml::Value::String("cid-1".into()));
        let mut parent = mk_span_node("parent", KindClass::InvokeAgent, None, 100, vec![]);
        parent.children.push(mk_span_node(
            "kid",
            KindClass::Chat,
            None,
            150,
            vec![],
        ));
        seed_session_tree(&app, "cid-1", vec![parent]);
        // Tall enough to enable the detail pane (height >= 16).
        let text = render_buf_text(&mut app, 120, 20);
        // Detail border title is " detail "
        assert!(text.contains("detail"), "no detail pane in:\n{text}");
        // Parent of focused root should be "—".
        assert!(text.contains("↑ parent"));
        assert!(text.contains("children"));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn pressing_s_opens_session_popover() {
        let mut app = one_spans_column_app();
        seed_sessions(
            &app,
            vec![SessionSummary {
                conversation_id: "abcdef0123456789".into(),
                first_seen_ns: None,
                last_seen_ns: None,
                latest_model: Some("gpt-x".into()),
                chat_turn_count: 0,
                tool_call_count: 0,
                agent_run_count: 0,
                service_name: None,
                local_name: None,
                user_named: None,
                cwd: None,
                branch: None,
            }],
        );
        use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};
        let k = KeyEvent {
            code: KeyCode::Char('s'),
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        };
        let _ = app.handle_key(k).unwrap();
        let col_id = app.workspace.columns[0].id.clone();
        assert_eq!(
            app.spans_scenario(&col_id).unwrap().state().popover,
            Some(SpansPopover::Session)
        );
        let text = render_buf_text(&mut app, 120, 20);
        // Popover header shows "Session"
        assert!(text.contains("Session"), "popover not rendered:\n{text}");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn pressing_k_opens_kind_popover_and_delete_clears_filter() {
        let mut app = one_spans_column_app();
        // Pre-set a kind_filter to test the clear path.
        app.workspace.columns[0]
            .config
            .insert("kind_filter".into(), toml::Value::String("chat".into()));
        use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};
        let k = KeyEvent {
            code: KeyCode::Char('k'),
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        };
        let _ = app.handle_key(k).unwrap();
        let col_id = app.workspace.columns[0].id.clone();
        assert_eq!(
            app.spans_scenario(&col_id).unwrap().state().popover,
            Some(SpansPopover::Kind)
        );
        // Delete clears the filter.
        let kdel = KeyEvent {
            code: KeyCode::Delete,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        };
        let _ = app.handle_key(kdel).unwrap();
        assert!(app.workspace.columns[0]
            .config
            .get("kind_filter")
            .is_none());
        assert!(app.spans_scenario(&col_id).unwrap().state().popover.is_none());
    }

    // ---- Phase 2: Context Growth Widget wiring ----

    fn seed_session_contexts(
        app: &App,
        cid: &str,
        snaps: Vec<crate::tui::model::ContextSnapshot>,
    ) {
        let resp = crate::tui::model::ListSessionContextsResponse {
            conversation_id: cid.into(),
            context_snapshots: snaps,
        };
        app.rt.cache.put(FetchedRecord {
            key: crate::tui::cache::qkey(["session-contexts", cid]),
            generation: 1,
            value: serde_json::to_value(resp).unwrap(),
            stale_after: std::time::Duration::from_secs(60),
        });
    }

    fn mk_snapshot(
        span_pk: i64,
        captured: i128,
        limit: i64,
        current: i64,
    ) -> crate::tui::model::ContextSnapshot {
        crate::tui::model::ContextSnapshot {
            ctx_pk: captured as i64,
            span_pk: Some(span_pk),
            captured_ns: captured,
            token_limit: Some(limit),
            current_tokens: Some(current),
            messages_length: None,
            input_tokens: Some(current / 2),
            output_tokens: Some(current / 4),
            cache_read_tokens: Some(current / 8),
            reasoning_tokens: Some(current / 8),
            source: None,
        }
    }

    fn chat_node(id: &str, span_pk: i64, start: i128) -> SpanNode {
        SpanNode {
            span_pk,
            trace_id: "trace-1".into(),
            span_id: id.into(),
            parent_span_id: None,
            name: id.into(),
            kind_class: KindClass::Chat,
            ingestion_state: "complete".into(),
            start_unix_ns: Some(start),
            end_unix_ns: Some(start + 10),
            projection: SpanProjection::default(),
            children: vec![],
        }
    }

    #[test]
    fn toggle_context_widget_flips_visibility_and_drops_focus() {
        let mut app = make_app();
        app.workspace.context_widget_visible = true;
        app.focus = Focus::Widget;
        app.toggle_context_widget();
        assert!(!app.workspace.context_widget_visible);
        assert!(
            !app.focus.is_widget(),
            "hiding must drop widget focus"
        );
        app.toggle_context_widget();
        assert!(app.workspace.context_widget_visible);
    }

    #[test]
    fn widget_height_clamps_to_floor_point_eight_of_term_height() {
        let mut app = make_app();
        app.workspace.context_widget_visible = true;
        app.term_size = (120, 40); // cap = floor(0.8*40) = 32
        app.workspace.context_widget_height_rows = 10;
        // Grow past the cap.
        app.adjust_widget_height(100);
        assert_eq!(app.workspace.context_widget_height_rows, 32);
        // Shrink past the floor.
        app.adjust_widget_height(-100);
        assert_eq!(app.workspace.context_widget_height_rows, 3);
    }

    #[test]
    fn cycle_focus_includes_widget_slot_when_visible() {
        let mut app = one_spans_column_app();
        app.workspace.context_widget_visible = true;
        app.focus_column(0);
        // One column + widget = 2 slots. Tab forward lands on the widget.
        app.cycle_focus(1);
        assert!(app.focus.is_widget(), "expected widget focus after column");
        assert_eq!(
            app.context_widget.bar_cursor,
            Some(0),
            "entering widget focus seeds bar cursor"
        );
        // Tab again wraps back to the column.
        app.cycle_focus(1);
        assert!(!app.focus.is_widget());
        assert_eq!(app.focus.column_idx(), Some(0));
    }

    #[test]
    fn cycle_focus_skips_widget_slot_when_hidden() {
        let mut app = one_spans_column_app();
        app.workspace.context_widget_visible = false;
        app.focus_column(0);
        app.cycle_focus(1);
        assert!(!app.focus.is_widget());
        assert_eq!(app.focus.column_idx(), Some(0));
    }

    /// LLR: `TUI Context widget participates in Tab focus cycle` — "Esc
    /// while focused, or hiding the widget, returns focus to the columns."
    /// With a column previously focused, widget Esc MUST restore that
    /// column as the focus.
    #[tokio::test(flavor = "current_thread")]
    async fn widget_esc_restores_focus_to_last_column() {
        let mut app = one_spans_column_app();
        app.workspace.context_widget_visible = true;
        // Establish column 0 as the focused column, then Tab into the widget.
        app.focus_column(0);
        app.cycle_focus(1);
        assert!(app.focus.is_widget(), "precondition: Tab lands on widget");
        // Esc on the widget releases.
        let _ = app.handle_key(press(KeyCode::Esc)).unwrap();
        assert_eq!(
            app.focus.column_idx(),
            Some(0),
            "Esc on widget must return focus to the previously-focused column"
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn widget_cursor_move_publishes_hover_pk() {
        let mut app = one_spans_column_app();
        app.workspace.context_widget_visible = true;
        app.workspace.columns[0]
            .config
            .insert("session".into(), toml::Value::String("cid-1".into()));
        let tree = vec![chat_node("a", 10, 100), chat_node("b", 20, 200)];
        seed_session_tree(&app, "cid-1", tree);
        seed_session_contexts(
            &app,
            "cid-1",
            vec![mk_snapshot(10, 150, 1000, 400), mk_snapshot(20, 250, 1000, 800)],
        );
        app.context_widget.bar_cursor = Some(0);
        app.widget_move_cursor(1);
        assert_eq!(app.context_widget.bar_cursor, Some(1));
        let pk = *app.hovered_chat_pk.read().unwrap();
        assert_eq!(pk, Some(20), "hover pk must follow the bar cursor");
        // Clamp at the right edge.
        app.widget_move_cursor(1);
        assert_eq!(app.context_widget.bar_cursor, Some(1));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn widget_enter_routes_selection_into_spans_column() {
        let mut app = one_spans_column_app();
        app.workspace.context_widget_visible = true;
        app.workspace.columns[0]
            .config
            .insert("session".into(), toml::Value::String("cid-1".into()));
        let tree = vec![chat_node("a", 10, 100), chat_node("b", 20, 200)];
        seed_session_tree(&app, "cid-1", tree);
        seed_session_contexts(
            &app,
            "cid-1",
            vec![mk_snapshot(10, 150, 1000, 400), mk_snapshot(20, 250, 1000, 800)],
        );
        app.focus = Focus::Widget;
        app.context_widget.bar_cursor = Some(1); // chat "b"
        app.widget_select_current();
        // Selection is routed synchronously through spans_pick — cursor
        // must have moved to the flat index of span "b" (index 1).
        let col_id = app.workspace.columns[0].id.clone();
        let st = app.spans_scenario(&col_id).unwrap().state();
        assert_eq!(st.cursor, 1);
        assert_eq!(st.focused_span_id.as_deref(), Some("b"));
    }

    #[test]
    fn find_prior_chat_span_picks_predecessor() {
        use crate::tui::model::{KindClass, SpanNode, SpanProjection};
        fn n(pk: i64, end: i128, kc: KindClass) -> SpanNode {
            SpanNode {
                span_pk: pk,
                trace_id: format!("t{pk}"),
                span_id: format!("s{pk}"),
                parent_span_id: None,
                name: "chat".into(),
                kind_class: kc,
                ingestion_state: "real".into(),
                start_unix_ns: Some(end - 1),
                end_unix_ns: Some(end),
                projection: SpanProjection::default(),
                children: Vec::new(),
            }
        }
        let tree = vec![
            n(1, 100, KindClass::Chat),
            n(2, 200, KindClass::Chat),
            n(3, 300, KindClass::Chat),
        ];
        // Current is pk=3, end=300 → prior is pk=2.
        let p = tree.find_prior_chat(3, Some(300), Some(299)).unwrap();
        assert_eq!(p.span_pk, 2);
        // Earliest chat has no prior.
        assert!(tree.find_prior_chat(1, Some(100), Some(99)).is_none());
    }

    // ---- Phase 0 (refactor): key-dispatch precedence golden masters ----
    //
    // Only invariants traceable to an LLR. The current handle_key impl has
    // janky behavior beyond what the LLRs specify (e.g. modal swallowing
    // every non-matching key); those are intentionally NOT pinned here so
    // the refactor is free to fix them.

    fn press(code: KeyCode) -> crossterm::event::KeyEvent {
        key_mod(code, KeyModifiers::NONE)
    }

    fn key_mod(
        code: KeyCode,
        modifiers: KeyModifiers,
    ) -> crossterm::event::KeyEvent {
        use ratatui::crossterm::event::{KeyEvent, KeyEventKind, KeyEventState};
        KeyEvent {
            code,
            modifiers,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        }
    }

    /// `Delete` while the search input is active must exit search mode AND
    /// clear the query (btop-style). `Esc` keeps the query.
    #[tokio::test(flavor = "current_thread")]
    async fn search_delete_clears_query_and_exits_input_mode() {
        let mut app = one_spans_column_app();
        let col_id = app.workspace.columns[0].id.clone();
        {
            let s = app.spans_scenario_mut(&col_id).unwrap().state_mut();
            s.search_active = true;
            s.search.set_text("hello");
            s.last_search_emitted = "hello".to_string();
        }
        let _ = app.handle_key(press(KeyCode::Delete)).unwrap();
        let s = app.spans_scenario(&col_id).unwrap().state();
        assert!(!s.search_active, "Delete must exit search-input mode");
        assert_eq!(s.search.text(), "", "Delete must clear the query");
        assert_eq!(
            s.last_search_emitted, "",
            "Delete must reset last_search_emitted so re-typing the same text re-emits",
        );
    }

    /// `Esc` while the search input is active exits input mode but preserves
    /// the query text — separate from `Delete`.
    #[tokio::test(flavor = "current_thread")]
    async fn search_esc_exits_input_mode_but_keeps_query() {
        let mut app = one_spans_column_app();
        let col_id = app.workspace.columns[0].id.clone();
        {
            let s = app.spans_scenario_mut(&col_id).unwrap().state_mut();
            s.search_active = true;
            s.search.set_text("hello");
            s.last_search_emitted = "hello".to_string();
        }
        let _ = app.handle_key(press(KeyCode::Esc)).unwrap();
        let s = app.spans_scenario(&col_id).unwrap().state();
        assert!(!s.search_active, "Esc must exit search-input mode");
        assert_eq!(s.search.text(), "hello", "Esc must preserve the query");
    }

    /// `Enter` / `Shift+Enter` cycle the cursor through server-side search
    /// matches in the visible flat list. Tested in column-focused mode
    /// (search input not active).
    #[tokio::test(flavor = "current_thread")]
    async fn search_enter_cycles_to_next_match_and_shift_enter_to_previous() {
        let mut app = one_spans_column_app();
        app.workspace.columns[0]
            .config
            .insert("session".into(), toml::Value::String("cid-1".into()));
        // Flat tree of 5 top-level spans. Hits will be set on a + c.
        let tree = vec![
            mk_span_node_named("a", "a", KindClass::ExecuteTool, Some("bash"), 10, vec![]),
            mk_span_node_named("b", "b", KindClass::ExecuteTool, Some("bash"), 20, vec![]),
            mk_span_node_named("c", "c", KindClass::ExecuteTool, Some("bash"), 30, vec![]),
            mk_span_node_named("d", "d", KindClass::ExecuteTool, Some("bash"), 40, vec![]),
            mk_span_node_named("e", "e", KindClass::ExecuteTool, Some("bash"), 50, vec![]),
        ];
        seed_session_tree(&app, "cid-1", tree);
        seed_search(&app, "cid-1", "q", vec!["a", "c"]);
        // Seed the column state to mirror the active-search precondition.
        let col_id = app.workspace.columns[0].id.clone();
        {
            let s = app.spans_scenario_mut(&col_id).unwrap().state_mut();
            s.search.set_text("q");
            s.last_search_emitted = "q".to_string();
            // Park the cursor between the two hits (idx 1 = "b").
            s.cursor = 1;
        }

        // Enter advances to the next match after cursor=1 → c (idx 2).
        let _ = app.handle_key(press(KeyCode::Enter)).unwrap();
        assert_eq!(app.spans_scenario(&col_id).unwrap().state().cursor, 2);

        // Enter again wraps past d/e back to a (idx 0).
        let _ = app.handle_key(press(KeyCode::Enter)).unwrap();
        assert_eq!(app.spans_scenario(&col_id).unwrap().state().cursor, 0);

        // Shift+Enter goes prev — from a, wraps to c (idx 2).
        let _ = app
            .handle_key(key_mod(KeyCode::Enter, KeyModifiers::SHIFT))
            .unwrap();
        assert_eq!(app.spans_scenario(&col_id).unwrap().state().cursor, 2);
    }

    /// When the cursor is already on a hit, Enter must skip it (vim `n`
    /// semantics).
    #[tokio::test(flavor = "current_thread")]
    async fn search_enter_skips_current_row_when_already_on_a_match() {
        let mut app = one_spans_column_app();
        app.workspace.columns[0]
            .config
            .insert("session".into(), toml::Value::String("cid-1".into()));
        let tree = vec![
            mk_span_node_named("a", "a", KindClass::ExecuteTool, Some("bash"), 10, vec![]),
            mk_span_node_named("b", "b", KindClass::ExecuteTool, Some("bash"), 20, vec![]),
            mk_span_node_named("c", "c", KindClass::ExecuteTool, Some("bash"), 30, vec![]),
        ];
        seed_session_tree(&app, "cid-1", tree);
        seed_search(&app, "cid-1", "q", vec!["a", "c"]);
        let col_id = app.workspace.columns[0].id.clone();
        {
            let s = app.spans_scenario_mut(&col_id).unwrap().state_mut();
            s.search.set_text("q");
            s.cursor = 0; // already on hit "a"
        }
        let _ = app.handle_key(press(KeyCode::Enter)).unwrap();
        assert_eq!(
            app.spans_scenario(&col_id).unwrap().state().cursor,
            2,
            "Enter on a hit must skip current and advance to next hit",
        );
    }

    /// With an empty query, Enter is a no-op (cursor unchanged) but still
    /// consumed so it doesn't trigger anything else.
    #[tokio::test(flavor = "current_thread")]
    async fn search_enter_no_op_when_query_empty() {
        let mut app = one_spans_column_app();
        app.workspace.columns[0]
            .config
            .insert("session".into(), toml::Value::String("cid-1".into()));
        let tree = vec![mk_span_node_named(
            "a",
            "a",
            KindClass::ExecuteTool,
            Some("bash"),
            10,
            vec![],
        )];
        seed_session_tree(&app, "cid-1", tree);
        let col_id = app.workspace.columns[0].id.clone();
        {
            let s = app.spans_scenario_mut(&col_id).unwrap().state_mut();
            s.cursor = 0;
        }
        let _ = app.handle_key(press(KeyCode::Enter)).unwrap();
        assert_eq!(app.spans_scenario(&col_id).unwrap().state().cursor, 0);
    }

    /// LLR: `TUI Tool detail key-dispatch precedence within column` —
    /// `Tab` / `Shift-Tab` are not column-local block navigation keys; they
    /// pass through the column layer so the global focus cycle can move
    /// between columns.
    #[test]
    fn precedence_tab_in_tool_detail_column_passes_to_global_focus_cycle() {
        use crate::tui::scenarios::tool_detail::{
            FocusKind, ToolDetailScenario, ToolDetailState,
        };

        let mut app = tool_detail_then_spans_app();
        let col_id = app.workspace.columns[0].id.clone();
        // Replace the auto-instantiated scenario with one carrying a
        // pre-seeded focus plan.
        app.scenarios.insert(
            col_id,
            Box::new(ToolDetailScenario {
                state: ToolDetailState {
                    focused_block: 0,
                    focus_plan: vec![
                        ("metadata".into(), FocusKind::Metadata),
                        ("body".into(), FocusKind::Search),
                    ],
                    ..Default::default()
                },
            }),
        );

        // `Dispatch::Pass` proves the column layer never invoked the
        // scenario's handle_key, so the focused_block cannot have moved.
        assert_eq!(app.layer_column(press(KeyCode::Tab)), Dispatch::Pass);

        let quit = app.handle_key(press(KeyCode::Tab)).unwrap();
        assert!(!quit);
        assert_eq!(app.focus.column_idx(), Some(1));
    }

    /// LLR: `TUI Spans search input edit semantics` — "Printable characters
    /// are inserted at the cursor verbatim." Combined with the
    /// `TUI key-dispatch precedence text-input > modal > widget > column > global`
    /// LLR (text-input layer 1 consumes printable), a `q` typed into the
    /// active search input MUST land in the buffer and MUST NOT reach the
    /// global-layer quit handler.
    #[test]
    fn precedence_q_during_active_spans_search_does_not_quit() {
        let mut app = one_spans_column_app();
        let col_id = app.workspace.columns[0].id.clone();
        app.spans_scenario_mut(&col_id).unwrap().state_mut()
            .search_active = true;
        let quit = app.handle_key(press(KeyCode::Char('q'))).unwrap();
        assert!(!quit, "q must not quit while search input has focus");
        let s = app.spans_scenario(&col_id).unwrap().state();
        assert!(s.search.text().contains('q'), "q must reach search input");
    }

    /// LLRs: `TUI Context widget keyboard bar cursor navigation` ("When the
    /// Context Growth Widget holds focus, `←`/`→` MUST move a keyboard bar
    /// cursor") + `TUI Context widget participates in Tab focus cycle`
    /// ("Widget-local keys (`←`/`→`/`Enter`/`Esc`) are dispatched in the
    /// precedence layer between modals and the focused column") +
    /// `TUI key-dispatch precedence text-input > modal > widget > column > global`.
    /// With the widget focused, `Right` MUST advance the widget's bar cursor
    /// and MUST NOT reach the focused column's scenario.
    #[test]
    fn precedence_widget_focus_consumes_arrow_keys_before_column() {
        let mut app = one_spans_column_app();
        app.workspace.context_widget_visible = true;
        app.workspace.columns[0]
            .config
            .insert("session".into(), toml::Value::String("cid-1".into()));
        let tree = vec![chat_node("a", 10, 100), chat_node("b", 20, 200)];
        seed_session_tree(&app, "cid-1", tree);
        seed_session_contexts(
            &app,
            "cid-1",
            vec![mk_snapshot(10, 150, 1000, 400), mk_snapshot(20, 250, 1000, 800)],
        );
        app.focus = Focus::Widget;
        app.context_widget.bar_cursor = Some(0);
        let col_id = app.workspace.columns[0].id.clone();
        let col_cursor_before = app.spans_scenario(&col_id).map(|s| s.state())
            .map(|s| s.cursor)
            .unwrap_or(0);
        let _ = app.handle_key(press(KeyCode::Right)).unwrap();
        assert_eq!(
            app.context_widget.bar_cursor,
            Some(1),
            "widget layer must consume Right before column"
        );
        let col_cursor_after = app.spans_scenario(&col_id).map(|s| s.state())
            .map(|s| s.cursor)
            .unwrap_or(0);
        assert_eq!(
            col_cursor_before, col_cursor_after,
            "column row cursor must not have moved"
        );
    }

    #[test]
    fn question_opens_keymap_overlay_not_logs() {
        let mut app = make_app();
        app.workspace.columns.clear();
        app.focus = Focus::None;
        let quit = app.handle_key(press(KeyCode::Char('?'))).unwrap();
        assert!(!quit);
        assert!(app.keymap_overlay_visible, "? must open the keymap overlay");
        assert!(!app.log_overlay_visible, "? must not open logs");

        let _ = app.handle_key(press(KeyCode::Char('?'))).unwrap();
        assert!(!app.keymap_overlay_visible, "? must close the keymap modal");
    }

    #[test]
    fn tilde_opens_log_overlay_not_keymap() {
        let mut app = make_app();
        app.workspace.columns.clear();
        app.focus = Focus::None;
        let quit = app.handle_key(press(KeyCode::Char('~'))).unwrap();
        assert!(!quit);
        assert!(app.log_overlay_visible, "~ must open the log overlay");
        assert!(!app.keymap_overlay_visible, "~ must not open keymap");

        let _ = app.handle_key(press(KeyCode::Char('~'))).unwrap();
        assert!(!app.log_overlay_visible, "~ must close the log modal");
    }

    #[test]
    fn keymap_for_spans_focus_includes_global_and_spans_keys() {
        let app = one_spans_column_app();
        let entries = app.active_keymap_entries();
        assert!(entries.iter().any(|(key, _)| key == "?"));
        assert!(entries.iter().any(|(key, _)| key == "~"));
        assert!(entries.iter().any(|(key, _)| key == "f"));
        assert!(entries.iter().any(|(key, _)| key == "s"));
        assert!(entries.iter().any(|(key, _)| key == "k"));
    }

    #[test]
    fn ctrl_c_event_quits_on_first_handle_call() {
        let mut app = one_spans_column_app();
        use ratatui::crossterm::event::{KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};
        let k = KeyEvent {
            code: KeyCode::Char('c'),
            modifiers: KeyModifiers::CONTROL,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        };
        let quit = app.handle_key(k).unwrap();
        assert!(quit, "Ctrl-C event must stop the event loop immediately");
    }

    /// LLR: `Keybinding Matrix` — Global row "`Ctrl-C` | Quit". No HLR
    /// scopes this conditionally; Ctrl-C MUST quit unconditionally at the
    /// global layer.
    #[test]
    fn precedence_ctrl_c_quits_even_with_focused_column() {
        let mut app = one_spans_column_app();
        use ratatui::crossterm::event::{KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};
        let k = KeyEvent {
            code: KeyCode::Char('c'),
            modifiers: KeyModifiers::CONTROL,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        };
        let quit = app.handle_key(k).unwrap();
        assert!(quit, "Ctrl-C must quit at the global layer");
    }

    #[test]
    fn find_prior_chat_span_ignores_non_chat_kinds() {
        use crate::tui::model::{KindClass, SpanNode, SpanProjection};
        fn n(pk: i64, end: i128, kc: KindClass) -> SpanNode {
            SpanNode {
                span_pk: pk,
                trace_id: format!("t{pk}"),
                span_id: format!("s{pk}"),
                parent_span_id: None,
                name: "x".into(),
                kind_class: kc,
                ingestion_state: "real".into(),
                start_unix_ns: Some(end - 1),
                end_unix_ns: Some(end),
                projection: SpanProjection::default(),
                children: Vec::new(),
            }
        }
        let tree = vec![
            n(1, 100, KindClass::Chat),
            n(2, 200, KindClass::ExecuteTool),
            n(3, 300, KindClass::Chat),
        ];
        let p = tree.find_prior_chat(3, Some(300), None).unwrap();
        assert_eq!(p.span_pk, 1);
    }
}
