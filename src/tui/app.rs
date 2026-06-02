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
    FetchPolicy, QueryCache, cache_get, qkey, swr_read, ws_invalidation_prefixes,
};
use crate::tui::model::{KindClass, SessionSpanTreeResponse, SpanTreeExt, WsEnvelope, WsKind};
use crate::tui::persist;
use crate::tui::scenarios::live_sessions::{
    clear_session_everywhere, delete_prompt, propagate_session,
};
use crate::tui::scenarios::render_placeholder;
use crate::tui::scenarios::spans::{
    SelectionPatch, SpansPopover, SpansState, attrs, chips, follow_chat, follow_mode,
    hovered_chat_ancestor, invoke_agent, propagate_search, propagate_selection,
};
use crate::tui::widgets::confirm_modal::{ConfirmModalState, ConfirmModalView};
use crate::tui::widgets::context_growth::{
    ContextGrowthState, ContextGrowthWidget, MergedRows, chat_span_pks, max_current_tokens,
    merge_snapshots,
};
use crate::tui::widgets::keymap_overlay::KeymapOverlay;
use crate::tui::widgets::kind_badge::kind_label;
use crate::tui::widgets::log_overlay::{LogBuffer, LogOverlay};
use crate::tui::widgets::select::{SelectPopover, SelectState};
use crate::tui::widgets::spans_tree_row::SpansTreeRow;
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
    /// Per-column Spans scenario state (not yet migrated to the
    /// `Scenario` trait — see `scenario-trait-spans` todo). All other
    /// scenarios live in `scenarios` below.
    pub spans_state: HashMap<String, SpansState>,
    /// Per-column `Scenario` instances (LiveSessions, ToolDetail,
    /// ChatDetail, FileTouches, …). Spans is special-cased in dispatch
    /// until its migration lands.
    pub scenarios: HashMap<String, Box<dyn crate::tui::scenarios::Scenario>>,
    /// Cross-column hovered chat pk store. Spans publishes; Phase 2 widget
    /// consumes.
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
    /// Memo for [`Self::cached_span_detail`]. Keyed by `(trace_id, span_id)`
    /// → `(cache_generation, Rc<SpanDetail>)`. Re-deserialized only when the
    /// cache generation for the span key changes; survives across draws so
    /// repeated lookups of an unchanged span (chips, report_intent walk,
    /// inspector pane, chat-detail prior lookup) cost a single HashMap hit.
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
            spans_state: HashMap::new(),
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

    /// Instantiate the `Scenario` for a given `ScenarioType`. Returns
    /// `None` for scenario types that are not yet trait-migrated (Spans,
    /// RawBrowser) — those go through legacy dispatch in App.
    fn scenario_for(t: ScenarioType) -> Option<Box<dyn crate::tui::scenarios::Scenario>> {
        use crate::tui::scenarios as sc;
        match t {
            ScenarioType::LiveSessions => Some(Box::new(sc::live_sessions::LiveSessionsScenario::new())),
            ScenarioType::ToolDetail => Some(Box::new(sc::tool_detail::ToolDetailScenario::new())),
            ScenarioType::ChatDetail => Some(Box::new(sc::chat_detail::ChatDetailScenario::new())),
            ScenarioType::FileTouches => Some(Box::new(sc::file_touches::FileTouchesScenario::new())),
            // Not yet migrated:
            ScenarioType::Spans => None,
            ScenarioType::RawBrowser => None,
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
            self.advance_follow_mode_columns();
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

    /// Drain due reveal-queue entries on every Spans column. Called by the
    /// event loop when its animation deadline fires.
    pub fn tick_anim(&mut self) {
        let now_ms = self.now_ms();
        for s in self.spans_state.values_mut() {
            let _ = s.reveal.drain_due(now_ms);
        }
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
        for s in self.spans_state.values() {
            if let Some(&(_, at)) = s.reveal.queue.first() {
                earliest = Some(earliest.map_or(at, |e| e.min(at)));
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

    /// For every Spans column with `follow_mode` engaged, advance the
    /// cursor to the latest tool span and propagate selection. Idempotent
    /// when no new latest tool span exists. Pulled out of the old per-tick
    /// loop and invoked from [`Self::on_ws_envelope`] for spans-touching
    /// envelopes.
    pub(crate) fn advance_follow_mode_columns(&mut self) {
        self.tick_follow_mode_advance();
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

    /// Layer 1 — text-input mode. Currently the only text-input target is
    /// the Spans column's search box (`/`-activated). Per
    /// `TUI Spans search input edit semantics`: printable characters and
    /// arrows go to the input verbatim; `Esc` exits to column-focused mode.
    fn layer_text_input(&mut self, k: crossterm::event::KeyEvent) -> Dispatch {
        let Some(i) = self.focus.column_idx() else {
            return Dispatch::Pass;
        };
        let col_id = self.workspace.columns[i].id.clone();
        if self.workspace.columns[i].scenario_type != ScenarioType::Spans {
            return Dispatch::Pass;
        }
        let active = self
            .spans_state
            .get(&col_id)
            .map(|s| s.search_active)
            .unwrap_or(false);
        if !active {
            return Dispatch::Pass;
        }
        // Esc exits search-input mode (does not propagate further).
        if matches!(k.code, KeyCode::Esc) {
            if let Some(s) = self.spans_state.get_mut(&col_id) {
                s.search_active = false;
            }
            return Dispatch::Consumed;
        }
        let mut emitted: Option<String> = None;
        if let Some(s) = self.spans_state.get_mut(&col_id) {
            if s.search.handle_key(k) {
                if s.search.take_changed() {
                    let t = s.search.text().to_string();
                    if t != s.last_search_emitted {
                        s.last_search_emitted = t.clone();
                        emitted = Some(t);
                    }
                }
                if let Some(query) = emitted {
                    propagate_search(&mut self.workspace.columns, &query);
                    let _ = persist::save(&self.workspace);
                    self.kick_search_debounce(&col_id, query);
                }
                return Dispatch::Consumed;
            }
        }
        Dispatch::Pass
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
            let popover = self
                .spans_state
                .get(&col_id)
                .and_then(|s| s.popover);
            if let Some(pk) = popover {
                if self.handle_spans_popover_key(i, &col_id, pk, k) {
                    return Dispatch::Consumed;
                }
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
        if self.spans_search_input_active() {
            return Self::text_input_keymap();
        }

        let mut entries = Self::global_keymap();
        match self.focus {
            Focus::None => {}
            Focus::Widget => entries.extend(Self::context_widget_keymap()),
            Focus::Column(i) => {
                if let Some(col) = self.workspace.columns.get(i) {
                    match col.scenario_type {
                        ScenarioType::Spans => entries.extend(Self::spans_keymap()),
                        _ => {
                            if let Some(scenario) = self.scenarios.get(&col.id) {
                                entries.extend(scenario.keymap_entries(&col.config));
                            }
                        }
                    }
                }
            }
        }
        entries
    }

    fn spans_search_input_active(&self) -> bool {
        let Some(i) = self.focus.column_idx() else {
            return false;
        };
        let Some(col) = self.workspace.columns.get(i) else {
            return false;
        };
        if col.scenario_type != ScenarioType::Spans {
            return false;
        }
        self.spans_state
            .get(&col.id)
            .map(|s| s.search_active)
            .unwrap_or(false)
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

    fn text_input_keymap() -> Vec<(String, String)> {
        vec![
            Self::keymap_entry("printable", "append character"),
            Self::keymap_entry("← / →", "move cursor"),
            Self::keymap_entry("Home / End", "jump cursor"),
            Self::keymap_entry("Backspace", "delete character left"),
            Self::keymap_entry("Delete", "delete character right"),
            Self::keymap_entry("Esc", "exit search input"),
        ]
    }

    fn context_widget_keymap() -> Vec<(String, String)> {
        vec![
            Self::keymap_entry("← / →", "move widget bar cursor"),
            Self::keymap_entry("Enter", "select current bar"),
            Self::keymap_entry("Esc", "release widget focus"),
        ]
    }

    fn spans_keymap() -> Vec<(String, String)> {
        vec![
            Self::keymap_entry("↑ / ↓", "move row cursor"),
            Self::keymap_entry("← / →", "collapse / expand focused row"),
            Self::keymap_entry("Home / End", "jump to top / bottom"),
            Self::keymap_entry("+ / -", "expand all / collapse all"),
            Self::keymap_entry("Space", "toggle focused row"),
            Self::keymap_entry("f", "toggle follow mode"),
            Self::keymap_entry("/", "focus search input"),
            Self::keymap_entry("s", "open session selector"),
            Self::keymap_entry("k", "open kind filter"),
            Self::keymap_entry("Enter", "select focused row"),
        ]
    }

    /// Dispatch a key to the focused column's scenario. Returns `true` if
    /// the key was consumed. Spans is still legacy-dispatched on App; every
    /// other scenario goes through the trait.
    fn scenario_handle_key(&mut self, col_idx: usize, k: crossterm::event::KeyEvent) -> bool {
        let st = self.workspace.columns[col_idx].scenario_type;
        let col_id = self.workspace.columns[col_idx].id.clone();
        if matches!(st, ScenarioType::Spans) {
            return self.spans_key(col_idx, &col_id, k);
        }
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
        drop(ctx);
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
        }
    }

    fn spans_key(
        &mut self,
        col_idx: usize,
        col_id: &str,
        k: crossterm::event::KeyEvent,
    ) -> bool {
        // Per `TUI Spans traces list mode`: when `column.config.session` is
        // unset, the body is the recent-traces list (cursor = traces_cursor,
        // Enter reserved for Phase 6). When set, the body is the session
        // span tree (cursor = state.cursor, Enter routes via spans_pick).
        let has_session = self.workspace.columns[col_idx]
            .config
            .get("session")
            .and_then(|v| v.as_str())
            .is_some_and(|s| !s.is_empty());

        // Common keys (route to popovers / search-input toggle) apply in
        // both modes — the popovers and the search input themselves are
        // mode-agnostic affordances.
        match k.code {
            KeyCode::Char('/') => {
                let s = self.spans_state.entry(col_id.to_string()).or_default();
                s.search_active = true;
                return true;
            }
            KeyCode::Char('s') => {
                let sessions = self.cached_sessions();
                let n_sessions = sessions.len();
                let current_cid = self.workspace.columns[col_idx]
                    .config
                    .get("session")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
                let cur = current_cid
                    .as_ref()
                    .and_then(|cid| {
                        sessions.iter().position(|x| &x.conversation_id == cid)
                    })
                    .unwrap_or(0);
                let s = self.spans_state.entry(col_id.to_string()).or_default();
                s.popover = Some(SpansPopover::Session);
                let safe_cursor = cur.min(n_sessions.saturating_sub(1));
                s.session_picker.open(safe_cursor);
                return true;
            }
            KeyCode::Char('k') => {
                let s = self.spans_state.entry(col_id.to_string()).or_default();
                s.popover = Some(SpansPopover::Kind);
                s.kind_picker.open(0);
                return true;
            }
            _ => {}
        }

        if has_session {
            self.spans_key_session_tree(col_idx, col_id, k)
        } else {
            self.spans_key_traces(col_id, k)
        }
    }

    /// Key dispatch for the no-session traces-list mode. Per
    /// `TUI Spans traces list mode`: arrow keys move the cursor; `Enter` is
    /// reserved for Phase 6 trace-pick and MUST be consumed (no-op) so it
    /// does not fall through to the global layer.
    fn spans_key_traces(&mut self, col_id: &str, k: crossterm::event::KeyEvent) -> bool {
        // Key handlers MUST NOT trigger network fetches — render owns the
        // SWR lifecycle. Peek the cached traces length read-only.
        let max = swr_read::<crate::tui::model::ListTracesResponse, _, _>(
            &self.rt.cache,
            qkey(["traces"]),
            std::time::Duration::from_secs(5),
            FetchPolicy::ReadOnly,
            || async move { unreachable!("ReadOnly policy never invokes the fetcher") },
        )
        .map(|r| r.traces.len())
        .unwrap_or(0);
        let s = self.spans_state.entry(col_id.to_string()).or_default();
        match k.code {
            KeyCode::Up => {
                s.traces_cursor = s.traces_cursor.saturating_sub(1);
                true
            }
            KeyCode::Down => {
                if s.traces_cursor + 1 < max {
                    s.traces_cursor += 1;
                }
                true
            }
            KeyCode::Home => {
                s.traces_cursor = 0;
                true
            }
            KeyCode::End => {
                s.traces_cursor = max.saturating_sub(1);
                true
            }
            // Phase 6 reserved; swallow so it does not quit / reach global.
            KeyCode::Enter => true,
            _ => false,
        }
    }

    /// Key dispatch for the session-span-tree body mode.
    fn spans_key_session_tree(
        &mut self,
        col_idx: usize,
        col_id: &str,
        k: crossterm::event::KeyEvent,
    ) -> bool {
        let tree = self.cached_session_tree(col_idx);
        let flat = tree.flatten_visible(
            &self
                .spans_state
                .get(col_id)
                .map(|s| s.user_collapsed.clone())
                .unwrap_or_default(),
        );
        let max = flat.len();
        let state = self.spans_state.entry(col_id.to_string()).or_default();
        match k.code {
            KeyCode::Up => {
                state.cursor = state.cursor.saturating_sub(1);
                self.publish_hovered_chat(col_idx);
                true
            }
            KeyCode::Down => {
                if state.cursor + 1 < max {
                    state.cursor += 1;
                }
                self.publish_hovered_chat(col_idx);
                true
            }
            KeyCode::Home => {
                state.cursor = 0;
                self.publish_hovered_chat(col_idx);
                true
            }
            KeyCode::End => {
                state.cursor = max.saturating_sub(1);
                self.publish_hovered_chat(col_idx);
                true
            }
            KeyCode::Left => {
                if let Some(id) = flat.get(state.cursor) {
                    state.user_collapsed.insert(id.clone());
                }
                true
            }
            KeyCode::Right => {
                if let Some(id) = flat.get(state.cursor) {
                    state.user_collapsed.remove(id);
                }
                true
            }
            KeyCode::Char(' ') => {
                if let Some(id) = flat.get(state.cursor) {
                    if state.user_collapsed.contains(id) {
                        state.user_collapsed.remove(id);
                    } else {
                        state.user_collapsed.insert(id.clone());
                    }
                }
                true
            }
            KeyCode::Char('+') => {
                state.user_collapsed.clear();
                true
            }
            KeyCode::Char('-') => {
                // Collapse all rows that have children.
                let mut all: Vec<String> = Vec::new();
                fn walk(n: &crate::tui::model::SpanNode, out: &mut Vec<String>) {
                    if !n.children.is_empty() {
                        out.push(n.span_id.clone());
                    }
                    for c in &n.children {
                        walk(c, out);
                    }
                }
                for r in tree.iter() {
                    walk(r, &mut all);
                }
                for id in all {
                    state.user_collapsed.insert(id);
                }
                true
            }
            KeyCode::Char('f') => {
                state.follow_mode = !state.follow_mode;
                true
            }
            KeyCode::Enter => {
                if let Some(id) = flat.get(state.cursor).cloned() {
                    self.spans_pick(col_idx, &id);
                }
                true
            }
            _ => false,
        }
    }

    /// Popover key dispatch (session selector / kind filter). Returns true
    /// if the key was consumed.
    fn handle_spans_popover_key(
        &mut self,
        col_idx: usize,
        col_id: &str,
        which: SpansPopover,
        k: crossterm::event::KeyEvent,
    ) -> bool {
        match which {
            SpansPopover::Session => {
                let options: Vec<crate::tui::model::SessionSummary> = self.cached_sessions();
                let max = options.len();
                let Some(s) = self.spans_state.get_mut(col_id) else {
                    return false;
                };
                match k.code {
                    KeyCode::Esc => {
                        s.session_picker.close();
                        s.popover = None;
                        true
                    }
                    KeyCode::Up => {
                        s.session_picker.move_cursor(-1, max);
                        true
                    }
                    KeyCode::Down => {
                        s.session_picker.move_cursor(1, max);
                        true
                    }
                    KeyCode::Enter => {
                        let cursor = s.session_picker.cursor;
                        s.session_picker.close();
                        s.popover = None;
                        if let Some(opt) = options.get(cursor) {
                            let cid = opt.conversation_id.clone();
                            propagate_session(
                                &mut self.workspace.columns,
                                &cid,
                                col_idx,
                            );
                            let _ = persist::save(&self.workspace);
                        }
                        true
                    }
                    _ => true, // swallow other keys while popover is open
                }
            }
            SpansPopover::Kind => {
                let options: &[&str] =
                    &["chat", "execute_tool", "external_tool", "invoke_agent", "other"];
                let max = options.len();
                let Some(s) = self.spans_state.get_mut(col_id) else {
                    return false;
                };
                match k.code {
                    KeyCode::Esc => {
                        s.kind_picker.close();
                        s.popover = None;
                        true
                    }
                    KeyCode::Up => {
                        s.kind_picker.move_cursor(-1, max);
                        true
                    }
                    KeyCode::Down => {
                        s.kind_picker.move_cursor(1, max);
                        true
                    }
                    KeyCode::Enter => {
                        let cursor = s.kind_picker.cursor;
                        s.kind_picker.close();
                        s.popover = None;
                        if let Some(opt) = options.get(cursor) {
                            self.workspace.columns[col_idx].config.insert(
                                "kind_filter".into(),
                                toml::Value::String((*opt).to_string()),
                            );
                            let _ = persist::save(&self.workspace);
                        }
                        true
                    }
                    KeyCode::Delete | KeyCode::Backspace => {
                        // Clear the kind filter.
                        self.workspace.columns[col_idx].config.remove("kind_filter");
                        s.kind_picker.close();
                        s.popover = None;
                        let _ = persist::save(&self.workspace);
                        true
                    }
                    _ => true,
                }
            }
        }
    }

    /// Realize selection routing for a picked span in the Spans column.
    fn spans_pick(&mut self, col_idx: usize, picked_span_id: &str) {
        let tree = self.cached_session_tree(col_idx);
        // Locate the picked node for trace_id + kind + tool_call_id.
        let Some(node) = tree.find_by_id(picked_span_id) else {
            return;
        };
        let picked_kind = node.kind_class;
        let picked = SelectionPatch {
            trace_id: node.trace_id.clone(),
            span_id: node.span_id.clone(),
            tool_call_id: node
                .projection
                .tool_call
                .as_ref()
                .and_then(|tc| tc.call_id.clone()),
        };
        let chat_route: Option<SelectionPatch> = match picked_kind {
            KindClass::ExecuteTool | KindClass::ExternalTool => {
                follow_chat::find_following_chat_span(&tree, picked_span_id).map(|(t, s)| {
                    SelectionPatch {
                        trace_id: t,
                        span_id: s,
                        tool_call_id: None,
                    }
                })
            }
            KindClass::InvokeAgent => {
                invoke_agent::latest_chat_descendant(&tree, picked_span_id).map(|(t, s)| {
                    SelectionPatch {
                        trace_id: t,
                        span_id: s,
                        tool_call_id: None,
                    }
                })
            }
            _ => None,
        };
        propagate_selection(
            &mut self.workspace.columns,
            picked_kind,
            picked,
            chat_route,
            col_idx,
        );
        // Auto-engage / disengage follow-mode per the LLR.
        let latest = follow_mode::latest_tool_span(&tree).map(|(_, sid)| sid);
        let col_id = self.workspace.columns[col_idx].id.clone();
        if let Some(state) = self.spans_state.get_mut(&col_id) {
            state.focused_span_id = Some(picked_span_id.to_string());
            if latest.as_deref() == Some(picked_span_id) {
                state.follow_mode = true;
            } else {
                state.follow_mode = false;
            }
        }
        let _ = persist::save(&self.workspace);
    }

    fn publish_hovered_chat(&self, col_idx: usize) {
        let tree = self.cached_session_tree(col_idx);
        let col_id = &self.workspace.columns[col_idx].id;
        let Some(state) = self.spans_state.get(col_id) else {
            return;
        };
        let flat = tree.flatten_visible(&state.user_collapsed);
        let pk = flat
            .get(state.cursor)
            .and_then(|id| hovered_chat_ancestor(&tree, id));
        if let Ok(mut g) = self.hovered_chat_pk.write() {
            *g = pk;
        }
    }

    /// 300 ms debounce kick for the server-side span search. Per the
    /// `TUI Spans search input edit semantics` LLR: rapid typing must not
    /// fan out into N HTTP requests. The cache's in-flight dedupe coalesces
    /// identical queries (different debounce calls with the same text) and
    /// generation-bumped invalidation drops superseded results.
    ///
    /// `cache_get` is the only fetcher of `["search-spans", session, q]` —
    /// render reads through [`Self::cached_search_hits`] with
    /// `FetchPolicy::ReadOnly` and never kicks its own fetch.
    fn kick_search_debounce(&mut self, col_id: &str, q: String) {
        self.spans_state
            .entry(col_id.to_string())
            .or_default()
            .search_nonce = self
            .spans_state
            .get(col_id)
            .map(|s| s.search_nonce)
            .unwrap_or(0)
            .wrapping_add(1);
        // Empty q clears search results without fetching.
        if q.is_empty() {
            if let Some(s) = self.spans_state.get_mut(col_id) {
                s.search_hits = None;
            }
            return;
        }
        let Some(session) = self
            .workspace
            .columns
            .iter()
            .find(|c| c.id == col_id)
            .and_then(|c| c.config.get("session"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
        else {
            return;
        };
        let cache = self.rt.cache.clone();
        let api = self.rt.api.clone();
        tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(300)).await;
            let _ = cache_get(
                &cache,
                qkey(["search-spans", &session, &q]),
                std::time::Duration::from_secs(5),
                || async move {
                    let r = api.search_spans(&q, &session, Some(200)).await?;
                    Ok::<Value, anyhow::Error>(serde_json::to_value(r)?)
                },
            )
            .await;
        });
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

    /// Read-or-fetch `["sessions"]` and return the parsed list. Synchronous
    /// from the renderer's point of view: returns whatever is cached and
    /// triggers a refetch when stale (stale-while-revalidate).
    fn cached_sessions(&self) -> Vec<crate::tui::model::SessionSummary> {
        let api = self.rt.api.clone();
        swr_read::<crate::tui::model::ListSessionsResponse, _, _>(
            &self.rt.cache,
            qkey(["sessions"]),
            std::time::Duration::from_secs(5),
            FetchPolicy::Swr,
            move || async move {
                let r = api.list_sessions(Some(50), None).await?;
                Ok::<Value, anyhow::Error>(serde_json::to_value(r)?)
            },
        )
        .map(|r| r.sessions)
        .unwrap_or_default()
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
    /// move the row cursor to the chat span (if visible in the flattened
    /// tree) and call [`Self::spans_pick`] for the standard selection
    /// routing.
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
        for (idx, col_id) in spans_cols {
            // Move the row cursor to the picked chat span when it is visible
            // in the column's current flatten (collapse-state respected).
            let col_tree = self.cached_session_tree(idx);
            let collapsed = self
                .spans_state
                .get(&col_id)
                .map(|s| s.user_collapsed.clone())
                .unwrap_or_default();
            let flat = col_tree.flatten_visible(&collapsed);
            if let Some(pos) = flat.iter().position(|id| id == &picked_sid) {
                self.spans_state
                    .entry(col_id.clone())
                    .or_default()
                    .cursor = pos;
            }
            self.spans_pick(idx, &picked_sid);
        }
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

    /// Read-or-fetch `["traces"]` for traces-list mode (when no session is
    /// configured). Stale-while-revalidate.
    fn cached_traces(&self) -> Vec<crate::tui::model::TraceSummary> {
        let api = self.rt.api.clone();
        swr_read::<crate::tui::model::ListTracesResponse, _, _>(
            &self.rt.cache,
            qkey(["traces"]),
            std::time::Duration::from_secs(5),
            FetchPolicy::Swr,
            move || async move {
                let r = api.list_traces(Some(50), None).await?;
                Ok::<Value, anyhow::Error>(serde_json::to_value(r)?)
            },
        )
        .map(|r| r.traces)
        .unwrap_or_default()
    }

    /// Read-or-fetch a single span's detail under `["span", trace_id, span_id]`
    /// (stale_after = 30 s). Returns `None` while the fetch is still in
    /// flight or if the response shape doesn't parse.
    /// Read the SpanDetail for `(trace_id, span_id)` through the query
    /// cache, memoising the deserialized value keyed by the cache entry's
    /// `generation`. Re-deserializes only when the cache entry changes
    /// (background fetch completion or WS invalidation). Survives across
    /// draws — see `span_detail_memo` on [`App`].
    fn cached_span_detail(
        &mut self,
        trace_id: &str,
        span_id: &str,
    ) -> Option<Rc<crate::tui::model::SpanDetail>> {
        let memo_key = (trace_id.to_string(), span_id.to_string());
        let cache_key = qkey(["span", trace_id, span_id]);

        // Memo hit when our stored generation matches the cache's current
        // generation. Stale-or-absent cache value → fall through and let
        // swr_read decide whether to refetch.
        let current_gen = self
            .rt
            .cache
            .peek(&cache_key)
            .value
            .as_ref()
            .map(|c| c.generation);
        if let (Some(g), Some((memo_g, rc))) =
            (current_gen, self.span_detail_memo.get(&memo_key))
        {
            if *memo_g == g {
                return Some(rc.clone());
            }
        }

        let api = self.rt.api.clone();
        let tid = trace_id.to_string();
        let sid = span_id.to_string();
        let parsed = swr_read::<crate::tui::model::SpanDetail, _, _>(
            &self.rt.cache,
            cache_key.clone(),
            std::time::Duration::from_secs(30),
            FetchPolicy::Swr,
            move || async move {
                let r = api.get_span(&tid, &sid).await?;
                Ok::<Value, anyhow::Error>(serde_json::to_value(r)?)
            },
        )?;
        let rc = Rc::new(parsed);
        // Re-peek post-`swr_read`; if a background fetch has already raced
        // ahead the generation may have bumped — store whatever the cache
        // now reports so the next lookup is a clean memo hit.
        let stored_gen = self
            .rt
            .cache
            .peek(&cache_key)
            .value
            .as_ref()
            .map(|c| c.generation)
            .unwrap_or(0);
        self.span_detail_memo.insert(memo_key, (stored_gen, rc.clone()));

        // Bound the memo loosely so it can't grow without limit on long
        // sessions. SPAN_LRU_CAP (1024) matches the cache's own span-key
        // cap; when we cross 2× that, drop everything and let lookups
        // repopulate. Coarse but predictable.
        if self.span_detail_memo.len() > 2 * crate::tui::cache::SPAN_LRU_CAP {
            self.span_detail_memo.clear();
        }
        Some(rc)
    }

    /// Read-only lookup of `["search-spans", session, q]`. The fetch
    /// lifecycle is owned by [`Self::kick_search_debounce`] (per the
    /// `TUI Spans search input edit semantics` LLR's 300 ms debounce
    /// contract) — render MUST NOT kick its own fetch here.
    fn cached_search_hits(
        &self,
        session: &str,
        q: &str,
    ) -> Option<crate::tui::model::SearchResponse> {
        if q.is_empty() {
            return None;
        }
        swr_read::<crate::tui::model::SearchResponse, _, _>(
            &self.rt.cache,
            qkey(["search-spans", session, q]),
            std::time::Duration::from_secs(5),
            FetchPolicy::ReadOnly,
            || async move { unreachable!("ReadOnly policy never invokes the fetcher") },
        )
    }

    /// Follow-mode advance hook (per `Spans follows latest tool span`).
    fn tick_follow_mode_advance(&mut self) {
        let col_ids: Vec<(usize, String)> = self
            .workspace
            .columns
            .iter()
            .enumerate()
            .filter(|(_, c)| c.scenario_type == ScenarioType::Spans)
            .map(|(i, c)| (i, c.id.clone()))
            .collect();
        for (col_idx, col_id) in col_ids {
            let follow = self
                .spans_state
                .get(&col_id)
                .map(|s| s.follow_mode)
                .unwrap_or(false);
            if !follow {
                continue;
            }
            let tree = self.cached_session_tree(col_idx);
            let Some((tid, sid)) = follow_mode::latest_tool_span(&tree) else {
                continue;
            };
            let current = self
                .spans_state
                .get(&col_id)
                .and_then(|s| s.focused_span_id.clone());
            if current.as_deref() == Some(sid.as_str()) {
                continue;
            }
            // Advance cursor to the new latest tool span.
            let collapsed = self
                .spans_state
                .get(&col_id)
                .map(|s| s.user_collapsed.clone())
                .unwrap_or_default();
            let flat = tree.flatten_visible(&collapsed);
            if let Some(new_idx) = flat.iter().position(|id| id == &sid) {
                if let Some(s) = self.spans_state.get_mut(&col_id) {
                    s.cursor = new_idx;
                    s.focused_span_id = Some(sid.clone());
                }
                // Propagate selection — same path as Enter (but without
                // disengaging follow-mode).
                let node = tree.find_by_id(&sid);
                if let Some(node) = node {
                    let picked = SelectionPatch {
                        trace_id: tid,
                        span_id: sid.clone(),
                        tool_call_id: node
                            .projection
                            .tool_call
                            .as_ref()
                            .and_then(|tc| tc.call_id.clone()),
                    };
                    let chat_route =
                        follow_chat::find_following_chat_span(&tree, &sid).map(
                            |(t, s)| SelectionPatch {
                                trace_id: t,
                                span_id: s,
                                tool_call_id: None,
                            },
                        );
                    propagate_selection(
                        &mut self.workspace.columns,
                        node.kind_class,
                        picked,
                        chat_route,
                        col_idx,
                    );
                }
            }
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
        self.focus_column(self.workspace.columns.len() - 1);
        self.sync_scenarios_with_workspace();
        let _ = persist::save(&self.workspace);
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
    ///
    /// On success, scrub the removed column's id from the spans-state map
    /// (the only legacy per-scenario state map left on `App` — all other
    /// scenarios live in `App::scenarios`, scrubbed by
    /// `sync_scenarios_with_workspace`).
    fn remove_focused_column(&mut self) {
        let Some(i) = self.focus.column_idx() else {
            return;
        };
        let Some(removed_id) = self.workspace.remove_column(i) else {
            return;
        };
        self.spans_state.remove(&removed_id);
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

        // Per-column dispatch. The Spans branch still needs `&mut self`
        // for legacy draw_spans, so each iteration re-reads the column
        // header off `&self.workspace.columns[i]` for that branch's call.
        // Non-Spans branches go through `self.scenarios.get_mut(&col_id)`
        // with a disjoint Ctx (api, cache, workspace, hovered_chat_pk,
        // span_detail_memo) — no clones of column state required.
        for i in 0..self.workspace.columns.len() {
            let rect = cols[i];
            let focused = self.focus.column_idx() == Some(i);
            // Snapshot just the scalars + ids needed by the legacy Spans
            // branch (its `&mut self` call invalidates any `&self.workspace`
            // borrow). For non-Spans branches we re-borrow `config` later
            // inside the disjoint window.
            let (col_id, title, st) = {
                let c = &self.workspace.columns[i];
                (c.id.clone(), c.title.clone(), c.scenario_type)
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
            match st {
                ScenarioType::Spans => {
                    // Legacy: takes &mut self; the function reads its own
                    // config off `self.workspace.columns[col_idx]`.
                    self.draw_spans(inner, buf, i, &col_id, outcome);
                }
                _ => {
                    // Disjoint-borrow Ctx: holds `&self.workspace` (so
                    // `&self.workspace.columns[i].config` is fine to alias
                    // for the scenario call).
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
                        render_placeholder(inner, buf, st, cfg);
                    }
                }
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

    fn draw_spans(
        &mut self,
        area: Rect,
        buf: &mut Buffer,
        col_idx: usize,
        col_id: &str,
        outcome: &mut DrawOutcome,
    ) {
        let cfg = &self.workspace.columns[col_idx].config;
        let session = cfg.get("session").and_then(|v| v.as_str()).map(str::to_string);
        let kind_filter: Option<String> = cfg
            .get("kind_filter")
            .and_then(|v| v.as_str())
            .map(str::to_string);
        // Two-row header.
        if area.height < 3 {
            return;
        }
        let header_h: u16 = 2;
        let h_top = Rect::new(area.x, area.y, area.width, 1);
        let h_bot = Rect::new(area.x, area.y + 1, area.width, 1);
        let body_total = Rect::new(area.x, area.y + header_h, area.width, area.height - header_h);

        // Row 1: session label.
        let sess_label = match &session {
            Some(s) => format!("session: {}", s.chars().take(8).collect::<String>()),
            None => "session: (none) — press 's'".to_string(),
        };
        Paragraph::new(Span::styled(sess_label, Style::default().fg(Color::Cyan)))
            .render(h_top, buf);

        // Snapshot SpansState scalars / cloned collections up front so the
        // immutable borrow on `self.spans_state` does not collide with the
        // `&mut self` calls (`draw_span_detail_pane`, `draw_spans_popover`,
        // `compute_row_chips`, `compute_report_intent_titles`,
        // `cached_search_hits`) later in this function.
        let snap = self.spans_state.get(col_id);
        let (cursor, follow_mode, search_active, search_text, user_collapsed) = match snap {
            Some(s) => (
                s.cursor,
                s.follow_mode,
                s.search_active,
                s.search.text().to_string(),
                s.user_collapsed.clone(),
            ),
            None => (0, false, false, String::new(), Default::default()),
        };

        // Row 2: kind / search / follow / collapse hints.
        let follow = if follow_mode { "[x] follow" } else { "[ ] follow" };
        let search_label = if search_active {
            format!("/ {search_text}_")
        } else if !search_text.is_empty() {
            format!("/ {search_text}")
        } else {
            "/  ".to_string()
        };
        let kf_label = kind_filter
            .as_deref()
            .map(|s| format!("k:{s}"))
            .unwrap_or_else(|| "k:kind".to_string());
        let hint = format!(
            "{kf_label}  {search_label}  {follow}  +/-:expand/collapse  s:session"
        );
        Paragraph::new(Span::styled(hint, Style::default().fg(Color::DarkGray)))
            .render(h_bot, buf);

        // No-session mode: render traces list.
        let Some(session) = session else {
            self.draw_traces_list(body_total, buf, col_id, outcome);
            self.draw_spans_popover(body_total, buf, col_id);
            return;
        };

        // Session mode: tree on top, span-detail inspector at the bottom.
        let detail_h: u16 = if body_total.height >= 12 { 6 } else { 0 };
        let tree_area = Rect::new(
            body_total.x,
            body_total.y,
            body_total.width,
            body_total.height.saturating_sub(detail_h),
        );
        let detail_area = Rect::new(
            body_total.x,
            body_total.y + body_total.height.saturating_sub(detail_h),
            body_total.width,
            detail_h,
        );

        // Render tree.
        let tree = self.cached_session_tree(col_idx);
        if tree.is_empty() {
            let dots = crate::tui::widgets::rolling_dots::frame_at(self.now_ms());
            outcome.spinner_visible = true;
            let line = Line::from(vec![
                Span::styled(
                    "loading spans".to_string(),
                    Style::default().fg(Color::DarkGray),
                ),
                Span::styled(dots.to_string(), Style::default().fg(Color::Yellow)),
            ]);
            Paragraph::new(line).render(tree_area, buf);
            self.draw_spans_popover(body_total, buf, col_id);
            return;
        }
        let flat = tree.flatten_visible(&user_collapsed);
        let visible_rows = tree_area.height as usize;
        let start = if cursor >= visible_rows {
            cursor + 1 - visible_rows
        } else {
            0
        };
        // Resolve search-hit set from the cache (server-side search).
        let hit_set: Option<std::collections::HashSet<String>> = if !search_text.is_empty() {
            self.cached_search_hits(&session, &search_text).map(|resp| {
                resp.results.into_iter().map(|r| r.span_id).collect()
            })
        } else {
            None
        };

        // Pre-compute per-parent report_intent titles (latest direct child with
        // tool_name == "report_intent" → intent string).
        let report_titles = self.compute_report_intent_titles(&tree);
        let kf_lower = kind_filter.as_deref().map(str::to_lowercase);

        for (i_visible, flat_idx) in (start..flat.len().min(start + visible_rows)).enumerate() {
            let row_id = &flat[flat_idx];
            let Some((node, depth)) = tree.find_with_depth(row_id) else {
                continue;
            };
            if node.ingestion_state == "placeholder" {
                // Mirror the condition in SpansTreeRow::render so the loop
                // can schedule the next dot-frame wake without inspecting
                // the rendered buffer.
                outcome.spinner_visible = true;
            }
            let row_y = tree_area.y + i_visible as u16;
            let focused = flat_idx == cursor;
            let (row_bg, mut row_dim) = match &hit_set {
                Some(set) if set.contains(&node.span_id) => (Some(Color::Yellow), false),
                Some(_) => (None, true),
                None => (None, false),
            };
            if let Some(kf) = &kf_lower {
                let cur = format!("{:?}", node.kind_class).to_lowercase();
                if cur != *kf {
                    row_dim = true;
                }
            }
            // `compute_row_chips` and the SpansTreeRow render want `node`
            // (an immutable reference into `tree`). Borrow scoping is fine
            // here — `tree` is a local Vec we own.
            let (chips, description) = self.compute_row_chips(node);
            let report_title = report_titles.get(&node.span_id).cloned();
            let row_area = Rect::new(tree_area.x, row_y, tree_area.width, 1);
            SpansTreeRow {
                node,
                depth,
                focused,
                collapsed: user_collapsed.contains(row_id),
                row_bg,
                row_dim,
                chips: &chips,
                description: description.as_deref(),
                report_title: report_title.as_deref(),
                now_ms: self.now_ms(),
            }
            .render(row_area, buf);
        }

        // Bottom detail inspector pane.
        if detail_h >= 3 {
            self.draw_span_detail_pane(detail_area, buf, &tree, &flat, cursor);
        }

        // Popover overlay (drawn over the body).
        self.draw_spans_popover(body_total, buf, col_id);
    }

    /// Render the no-session traces list. Implements `Traces list dims rows
    /// below kind filter`.
    fn draw_traces_list(
        &mut self,
        area: Rect,
        buf: &mut Buffer,
        col_id: &str,
        outcome: &mut DrawOutcome,
    ) {
        let traces = self.cached_traces();
        let default = SpansState::default();
        let st: &SpansState = self.spans_state.get(col_id).unwrap_or(&default);
        if traces.is_empty() {
            let dots = crate::tui::widgets::rolling_dots::frame_at(self.now_ms());
            outcome.spinner_visible = true;
            let line = Line::from(vec![
                Span::styled(
                    "loading traces".to_string(),
                    Style::default().fg(Color::DarkGray),
                ),
                Span::styled(dots.to_string(), Style::default().fg(Color::Yellow)),
            ]);
            Paragraph::new(line).render(area, buf);
            return;
        }
        let kind_filter: Option<String> = self.workspace.columns[
            self.workspace.columns.iter().position(|c| c.id == col_id).unwrap_or(0)
        ]
            .config
            .get("kind_filter")
            .and_then(|v| v.as_str())
            .map(str::to_string);
        let visible_rows = area.height as usize;
        let cursor = st.traces_cursor.min(traces.len().saturating_sub(1));
        let start = if cursor >= visible_rows {
            cursor + 1 - visible_rows
        } else {
            0
        };
        for (i_visible, idx) in (start..traces.len().min(start + visible_rows)).enumerate() {
            let t = &traces[idx];
            let id8: String = t.trace_id.chars().take(8).collect();
            let when = crate::tui::format::fmt_relative(
                t.last_seen_ns.map(|n| n as i128),
                None,
            );
            let counts = format!(
                "chat:{} tool:{} ext:{} agent:{} other:{}",
                t.kind_counts.chat,
                t.kind_counts.execute_tool,
                t.kind_counts.external_tool,
                t.kind_counts.invoke_agent,
                t.kind_counts.other,
            );
            let row = format!("{id8}  {when}  spans:{}  {counts}", t.span_count);
            let mut style = if idx == cursor {
                Style::default()
                    .bg(Color::Cyan)
                    .fg(Color::Black)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };
            // Kind filter dim: when filter set and the row has 0 of that kind.
            if let Some(kf) = &kind_filter {
                let count = match kf.as_str() {
                    "chat" => t.kind_counts.chat,
                    "execute_tool" => t.kind_counts.execute_tool,
                    "external_tool" => t.kind_counts.external_tool,
                    "invoke_agent" => t.kind_counts.invoke_agent,
                    "other" => t.kind_counts.other,
                    _ => 1,
                };
                if count == 0 {
                    style = style.add_modifier(Modifier::DIM);
                }
            }
            let row_y = area.y + i_visible as u16;
            buf.set_span(area.x, row_y, &Span::styled(row, style), area.width);
        }
    }

    /// Bottom span-detail inspector pane — parent, children, projection.
    fn draw_span_detail_pane(
        &mut self,
        area: Rect,
        buf: &mut Buffer,
        tree: &[crate::tui::model::SpanNode],
        flat: &[String],
        cursor: usize,
    ) {
        // Border
        let block = Block::default()
            .borders(Borders::TOP)
            .title(" detail ")
            .border_style(Style::default().fg(Color::DarkGray));
        let inner = block.inner(area);
        block.render(area, buf);
        if inner.height == 0 || inner.width < 8 {
            return;
        }
        let Some(focused_id) = flat.get(cursor) else {
            return;
        };
        let Some(node) = tree.find_by_id(focused_id) else {
            return;
        };
        // Try the cache for full detail; fall back to in-tree node for parent/children.
        let detail = self.cached_span_detail(&node.trace_id, &node.span_id);
        let mut lines: Vec<Line<'static>> = Vec::new();
        let id8: String = node.span_id.chars().take(8).collect();
        let head = format!(
            "{}  [{}]  {}",
            node.name,
            kind_label(node.kind_class),
            id8
        );
        lines.push(Line::from(Span::styled(
            head,
            Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
        )));

        // Parent + children. Prefer detail (server) when available.
        let parent_label = if let Some(d) = &detail {
            match &d.parent {
                Some(p) => format!(
                    "↑ parent: {} ({})",
                    p.name,
                    p.span_id.chars().take(8).collect::<String>()
                ),
                None => "↑ parent: —".to_string(),
            }
        } else if let Some(parent_id) = &node.parent_span_id {
            format!("↑ parent: {}", parent_id.chars().take(8).collect::<String>())
        } else {
            "↑ parent: —".to_string()
        };
        lines.push(Line::from(Span::styled(
            parent_label,
            Style::default().fg(Color::Cyan),
        )));

        let child_refs: Vec<(String, String, String)> = if let Some(d) = &detail {
            d.children
                .iter()
                .map(|c| (c.name.clone(), format!("{:?}", c.kind_class), c.span_id.clone()))
                .collect()
        } else {
            node.children
                .iter()
                .map(|c| (c.name.clone(), format!("{:?}", c.kind_class), c.span_id.clone()))
                .collect()
        };
        if child_refs.is_empty() {
            lines.push(Line::from(Span::styled(
                "↓ children: (none)",
                Style::default().fg(Color::DarkGray),
            )));
        } else {
            lines.push(Line::from(Span::styled(
                format!("↓ children ({}):", child_refs.len()),
                Style::default().fg(Color::Cyan),
            )));
            let take_n = (inner.height as usize).saturating_sub(3).min(child_refs.len()).max(0);
            for (name, kind, id) in child_refs.iter().take(take_n) {
                let id8: String = id.chars().take(8).collect();
                lines.push(Line::from(format!("  • {name} [{kind}] {id8}")));
            }
        }

        // Projection summary
        if let Some(d) = &detail {
            let p = &d.projection;
            let present: Vec<&str> = [
                p.chat_turn.as_ref().map(|_| "chat_turn"),
                p.tool_call.as_ref().map(|_| "tool_call"),
                p.agent_run.as_ref().map(|_| "agent_run"),
                p.external_tool_call.as_ref().map(|_| "external_tool_call"),
            ]
            .into_iter()
            .flatten()
            .collect();
            if !present.is_empty() {
                lines.push(Line::from(Span::styled(
                    format!("projection: {}", present.join(", ")),
                    Style::default().fg(Color::Magenta),
                )));
            }
        } else {
            lines.push(Line::from(Span::styled(
                "(loading detail…)",
                Style::default()
                    .fg(Color::DarkGray)
                    .add_modifier(Modifier::ITALIC),
            )));
        }
        Paragraph::new(lines).render(inner, buf);
    }

    /// Compute per-row chips (text, bg color) using cached per-span details.
    /// Returns chips + an optional plain-white description label that the
    /// renderer paints separately (no chip styling, per the LLR).
    fn compute_row_chips(
        &mut self,
        node: &crate::tui::model::SpanNode,
    ) -> (Vec<(String, Color)>, Option<String>) {
        let mut out: Vec<(String, Color)> = Vec::new();
        if !node.is_tool_row() {
            return (out, None);
        }
        let tool_name = node.projected_tool_name().unwrap_or_default().to_string();
        if !tool_name.is_empty() {
            out.push((tool_name.clone(), crate::tui::format::hash_color(&tool_name)));
        }
        let Some(detail) = self.cached_span_detail(&node.trace_id, &node.span_id) else {
            return (out, None);
        };
        let Some(attrs_v) = &detail.span.attributes else {
            return (out, None);
        };
        let Some(args) = attrs::parse_tool_call_arguments(attrs_v) else {
            return (out, None);
        };

        // Skill chip
        if tool_name == "skill" {
            if let Some(s) = chips::skill_chip(&args) {
                out.push((s, Color::Green));
            }
        }
        let kind_opt = crate::tui::vendor::copilot::tool_name_mapping(&tool_name);
        for target in chips::target_chips(kind_opt, &args) {
            out.push((target, Color::Cyan));
        }
        // Diff-stat badges
        if let Some(kind) = kind_opt {
            let (added, removed) = chips::diff_stat(kind, &args);
            if removed > 0 {
                out.push((format!("-{removed}"), Color::Red));
            }
            if added > 0 {
                out.push((format!("+{added}"), Color::Green));
            }
            // Shell chips
            if matches!(kind, crate::tui::vendor::copilot::ToolKind::Shell) {
                if let Some(cmd) = args.get("command").and_then(|v| v.as_str()) {
                    let mut shell_chips = chips::shell_command_chips(cmd);
                    if shell_chips.len() > 6 {
                        shell_chips.truncate(6);
                        shell_chips.push("…".to_string());
                    }
                    for c in shell_chips {
                        let color = crate::tui::format::hash_color(&c);
                        out.push((c, color));
                    }
                }
            }
        }
        // Tool description label — per `Spans tool description inline label`,
        // rendered as plain white text with no chip styling. Kept separate
        // from the `chips` Vec so the renderer can paint it without forcing
        // a (bg, black-fg) chip style that would render black-on-black.
        let description = chips::tool_description_label(&args);
        (out, description)
    }

    /// Per `Report intent title shows on parent row` — for each node, look at
    /// its direct children for `tool_call.tool_name == "report_intent"`,
    /// pick the latest by `start_unix_ns ?? span_pk`, fetch its detail and
    /// parse `args.intent`. Returns `parent.span_id -> intent`.
    fn compute_report_intent_titles(
        &mut self,
        tree: &[crate::tui::model::SpanNode],
    ) -> std::collections::HashMap<String, String> {
        let mut out: std::collections::HashMap<String, String> = std::collections::HashMap::new();
        fn walk(
            node: &crate::tui::model::SpanNode,
            app: &mut App,
            out: &mut std::collections::HashMap<String, String>,
        ) {
            // Find latest report_intent direct child.
            let mut best: Option<&crate::tui::model::SpanNode> = None;
            let mut best_key: i128 = i128::MIN;
            for c in &node.children {
                let name = c
                    .projection
                    .tool_call
                    .as_ref()
                    .and_then(|t| t.tool_name.as_deref());
                if name != Some("report_intent") {
                    continue;
                }
                let key = c.start_unix_ns.unwrap_or(c.span_pk as i128);
                if key > best_key {
                    best_key = key;
                    best = Some(c);
                }
            }
            if let Some(child) = best {
                if let Some(detail) = app.cached_span_detail(&child.trace_id, &child.span_id) {
                    if let Some(attrs_v) = &detail.span.attributes {
                        if let Some(args) = attrs::parse_tool_call_arguments(attrs_v) {
                            if let Some(intent) = chips::report_intent_title(&args) {
                                out.insert(node.span_id.clone(), intent);
                            }
                        }
                    }
                }
            }
            for c in &node.children {
                walk(c, app, out);
            }
        }
        for r in tree {
            walk(r, self, &mut out);
        }
        out
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

    /// Popover overlay for the `s` (session) and `k` (kind) keys.
    fn draw_spans_popover(&mut self, area: Rect, buf: &mut Buffer, col_id: &str) {
        let Some(st) = self.spans_state.get(col_id) else {
            return;
        };
        let Some(which) = st.popover else { return };
        match which {
            SpansPopover::Session => {
                let sessions = self.cached_sessions();
                let options: Vec<String> = sessions
                    .iter()
                    .map(|s| {
                        let id8: String = s.conversation_id.chars().take(8).collect();
                        let model = s.latest_model.as_deref().unwrap_or("—");
                        format!("{id8}  {model}")
                    })
                    .collect();
                SelectPopover {
                    title: "Session",
                    options: &options,
                    cursor: st.session_picker.cursor,
                }
                .render(area, buf);
            }
            SpansPopover::Kind => {
                let options: Vec<String> = [
                    "chat",
                    "execute_tool",
                    "external_tool",
                    "invoke_agent",
                    "other",
                ]
                .iter()
                .map(|s| (*s).to_string())
                .collect();
                SelectPopover {
                    title: "Kind filter (Delete to clear)",
                    options: &options,
                    cursor: st.kind_picker.cursor,
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
                // Some background fetch populated (or invalidated) a cache
                // entry visible to the renderer; just request a redraw.
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
    use crate::tui::scenarios::spans::{SpansPopover, SpansState};

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
        app.rt.cache.put(FetchedRecord {
            key: crate::tui::cache::qkey(["session-span-tree", cid]),
            generation: 1,
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
        row_text(buf, y).find(needle).map(|x| x as u16)
    }

    fn one_spans_column_app() -> App {
        let mut app = make_app();
        app.workspace.columns.clear();
        app.workspace
            .add_column(crate::tui::workspace::ScenarioType::Spans);
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
            buf[(x, 4)].style().bg,
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
            buf[(x, 4)].style().bg,
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
        assert_eq!(buf[(x, 4)].style().bg, Some(Color::Cyan));
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
        assert_eq!(buf[(x, 4)].style().bg, Some(Color::Cyan));
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
        app.spans_state
            .entry(col_id.clone())
            .or_insert_with(SpansState::new)
            .follow_mode = true;
        // Drive a spans-touching WS envelope (replaces the old Tick drive).
        app.on_ws_envelope(WsEnvelope {
            kind: WsKind::Span,
            entity: crate::tui::model::WsEntity::Span,
            payload: json!({}),
        });
        // Latest tool span is "tool-b" (end_ns=200) at flat index 2.
        let st = app.spans_state.get(&col_id).unwrap();
        assert_eq!(st.cursor, 2);
        assert_eq!(st.focused_span_id.as_deref(), Some("tool-b"));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn server_search_hit_highlights_matching_rows() {
        let mut app = one_spans_column_app();
        app.workspace.columns[0]
            .config
            .insert("session".into(), toml::Value::String("cid-1".into()));
        // Use Chat-kind nodes so the row name renders (tool-kind names are
        // now suppressed because chips carry the tool identity).
        let tree = vec![
            mk_span_node("hit-span", KindClass::Chat, None, 100, vec![]),
            mk_span_node("miss-span", KindClass::Chat, None, 200, vec![]),
        ];
        seed_session_tree(&app, "cid-1", tree);
        let col_id = app.workspace.columns[0].id.clone();
        let s = app.spans_state.entry(col_id).or_insert_with(SpansState::new);
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
            app.spans_state
                .get(&col_id)
                .map(|s| s.traces_cursor)
                .unwrap_or(0),
            0
        );
        let _ = app.handle_key(press(KeyCode::Down)).unwrap();
        assert_eq!(app.spans_state.get(&col_id).unwrap().traces_cursor, 1);
        let _ = app.handle_key(press(KeyCode::Down)).unwrap();
        assert_eq!(app.spans_state.get(&col_id).unwrap().traces_cursor, 2);
        // Clamps at last row.
        let _ = app.handle_key(press(KeyCode::Down)).unwrap();
        assert_eq!(app.spans_state.get(&col_id).unwrap().traces_cursor, 2);
        // Up wraps no further than 0.
        let _ = app.handle_key(press(KeyCode::Up)).unwrap();
        let _ = app.handle_key(press(KeyCode::Up)).unwrap();
        let _ = app.handle_key(press(KeyCode::Up)).unwrap();
        let _ = app.handle_key(press(KeyCode::Up)).unwrap();
        assert_eq!(app.spans_state.get(&col_id).unwrap().traces_cursor, 0);
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
            app.spans_state.get(&col_id).unwrap().popover,
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
            app.spans_state.get(&col_id).unwrap().popover,
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
        assert!(app.spans_state.get(&col_id).unwrap().popover.is_none());
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
        let st = app.spans_state.get(&col_id).unwrap();
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
        app.spans_state
            .entry(col_id.clone())
            .or_insert_with(SpansState::new)
            .search_active = true;
        let quit = app.handle_key(press(KeyCode::Char('q'))).unwrap();
        assert!(!quit, "q must not quit while search input has focus");
        let s = app.spans_state.get(&col_id).unwrap();
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
        let col_cursor_before = app
            .spans_state
            .get(&col_id)
            .map(|s| s.cursor)
            .unwrap_or(0);
        let _ = app.handle_key(press(KeyCode::Right)).unwrap();
        assert_eq!(
            app.context_widget.bar_cursor,
            Some(1),
            "widget layer must consume Right before column"
        );
        let col_cursor_after = app
            .spans_state
            .get(&col_id)
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
