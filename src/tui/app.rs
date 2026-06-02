//! Top-level TUI App: state, event-loop, draw. The loop obeys the
//! drain-then-draw rule: every pending [`AppEvent`] is drained via
//! `try_recv` before `terminal.draw` runs once.

use std::collections::HashMap;
use std::cell::RefCell;
use std::sync::{Arc, RwLock};

use anyhow::Result;
use ratatui::DefaultTerminal;
use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Widget};
use ratatui::crossterm::event::{
    DisableMouseCapture, EnableMouseCapture, KeyCode, KeyModifiers,
};
use ratatui::crossterm::execute;
use serde_json::Value;
use tokio::sync::mpsc;
use tracing::{debug, info};

use crate::tui::api::ApiClient;
use crate::tui::cache::{FetchPolicy, QueryCache, cache_get, qkey, swr_read};
use crate::tui::event::{
    AppEvent, spawn_crossterm_reader, spawn_tick, spawn_ws_coalescer,
};
use crate::tui::live_feed::LiveFeed;
use crate::tui::model::{KindClass, SessionSpanTreeResponse, SpanTreeExt};
use crate::tui::persist;
use crate::tui::scenarios::live_sessions::{
    LiveSessionsState, clear_session_everywhere, delete_prompt, propagate_session, render_row,
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
use crate::tui::widgets::kind_badge::{KindBadge, kind_label};
use crate::tui::widgets::log_overlay::{LogBuffer, LogOverlay};
use crate::tui::widgets::status_dot::StatusDot;
use crate::tui::workspace::{ScenarioType, Workspace};
use crate::tui::ws::{WsBus, WsStatus};

/// Minimum cell width for a column body (per terminal-rendering-constraints
/// LLR; analog of the web's `MIN_COL_PX = 280`).
pub const MIN_COL: u16 = 24;

/// Top-level app state.
pub struct App {
    pub workspace: Workspace,
    pub cache: Arc<QueryCache>,
    #[allow(dead_code)]
    pub live_feed: Arc<LiveFeed>,
    pub api: ApiClient,
    pub ws: WsBus,
    pub log_buffer: LogBuffer,
    pub focused_column: Option<usize>,
    pub log_overlay_visible: bool,
    pub mouse_enabled: bool,
    pub add_column_cursor: usize,
    pub status: WsStatus,
    pub last_ws_event: Option<String>,
    /// Per-column scenario state (keyed by column id).
    pub live_sessions_state: HashMap<String, LiveSessionsState>,
    pub spans_state: HashMap<String, SpansState>,
    /// Per-column tool-detail scenario state (keyed by column id). `RefCell`
    /// because `draw` takes `&self` but the searchable body blocks mutate
    /// state (search phase, scroll, focus plan) during render.
    pub tool_detail_state: RefCell<HashMap<String, crate::tui::scenarios::tool_detail::ToolDetailState>>,
    /// Per-column chat-detail scenario state.
    pub chat_detail_state: RefCell<HashMap<String, crate::tui::scenarios::chat_detail::ChatDetailState>>,
    /// Per-column chat-detail mode (DELTA / FULL). RefCell because `m` toggles
    /// it from `draw` indirectly (through the rendered state map) but the
    /// authoritative value lives here so `handle_key` can mutate it before
    /// the next draw.
    pub chat_detail_mode: RefCell<HashMap<String, crate::tui::scenarios::chat_detail::tree::ChatMode>>,
    /// Per-column file-touches scenario state.
    pub file_touches_state: RefCell<HashMap<String, crate::tui::scenarios::file_touches::FileTouchesState>>,
    /// Cross-column hovered chat pk store. Spans publishes; Phase 2 widget
    /// consumes.
    pub hovered_chat_pk: Arc<RwLock<Option<i64>>>,
    pub confirm_modal: ConfirmModalState,
    /// Records what session id the pending confirm-delete refers to (none
    /// when no confirm is open).
    pub pending_delete: Option<String>,
    /// Monotonic tick counter for animations.
    pub anim_tick: u64,
    /// Context Growth Widget keyboard-cursor state (Phase 2).
    pub context_widget: ContextGrowthState,
    /// True when keyboard focus is on the Context Growth Widget rather than a
    /// column (part of the `Tab` focus cycle when the widget is visible).
    pub widget_focused: bool,
    /// Last known terminal size `(width, height)`. Updated on resize and at
    /// startup; drives the widget-height `0.8 * term_h` clamp.
    pub term_size: (u16, u16),
}

impl App {
    pub fn new(api: ApiClient, ws: WsBus, log_buffer: LogBuffer, mouse_enabled: bool) -> Self {
        let workspace = persist::load();
        let focused_column = (!workspace.columns.is_empty()).then_some(0);
        Self {
            workspace,
            cache: Arc::new(QueryCache::new()),
            live_feed: Arc::new(LiveFeed::new()),
            api,
            status: ws.status(),
            ws,
            log_buffer,
            focused_column,
            log_overlay_visible: false,
            mouse_enabled,
            add_column_cursor: 0,
            last_ws_event: None,
            live_sessions_state: HashMap::new(),
            spans_state: HashMap::new(),
            tool_detail_state: RefCell::new(HashMap::new()),
            chat_detail_state: RefCell::new(HashMap::new()),
            chat_detail_mode: RefCell::new(HashMap::new()),
            file_touches_state: RefCell::new(HashMap::new()),
            hovered_chat_pk: Arc::new(RwLock::new(None)),
            confirm_modal: ConfirmModalState::new(),
            pending_delete: None,
            anim_tick: 0,
            context_widget: ContextGrowthState::default(),
            widget_focused: false,
            term_size: (0, 0),
        }
    }

    /// Process one drained event. Returns `Ok(true)` if the loop should
    /// quit.
    pub fn handle(&mut self, ev: AppEvent) -> Result<bool> {
        match ev {
            AppEvent::Quit => return Ok(true),
            AppEvent::Tick => {
                self.anim_tick = self.anim_tick.wrapping_add(1);
                // Drain reveal queues on every tick (per
                // `TUI Reveal schedule advances on every tick`).
                let now_ms = self.now_ms();
                for s in self.spans_state.values_mut() {
                    let _ = s.reveal.drain_due(now_ms);
                }
                // Follow-mode auto-advance: for every Spans column whose
                // follow_mode is on, recompute the latest tool span and
                // jump the cursor + propagate selection if it differs.
                self.tick_follow_mode_advance();
                // Phase 2: consume any pending Context Growth Widget bar
                // clicks (`Context widget bar click selects chat in Spans
                // column`) and refresh the widget's row count for cursor
                // clamping.
                self.consume_widget_clicks();
                self.context_widget.last_visible_rows = self
                    .widget_merged_context()
                    .map(|(m, _, _)| m.rows.len() as u16)
                    .unwrap_or(0);
            }
            AppEvent::Crossterm(crossterm::event::Event::Key(k))
                if k.kind == crossterm::event::KeyEventKind::Press =>
            {
                if self.handle_key(k)? {
                    return Ok(true);
                }
            }
            AppEvent::Crossterm(crossterm::event::Event::Resize(w, h)) => {
                self.term_size = (w, h);
            }
            AppEvent::Crossterm(_) => {}
            AppEvent::WsTick {
                dirty_prefixes,
                envelopes,
            } => {
                for env in &envelopes {
                    self.live_feed.ingest(env.clone());
                    self.last_ws_event =
                        Some(format!("{:?}/{:?}", env.kind, env.entity));
                }
                for p in &dirty_prefixes {
                    let segs: Vec<&str> = p.iter().map(String::as_str).collect();
                    self.cache.invalidate(&segs);
                }
                debug!(
                    envelopes = envelopes.len(),
                    prefixes = dirty_prefixes.len(),
                    "ws tick processed"
                );
                self.status = self.ws.status();
            }
            AppEvent::QueryResult { .. } => {}
        }
        Ok(false)
    }

    fn now_ms(&self) -> u64 {
        use std::time::{SystemTime, UNIX_EPOCH};
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0)
    }

    fn handle_key(&mut self, k: crossterm::event::KeyEvent) -> Result<bool> {
        // Precedence layer 2: log overlay (modal).
        if self.log_overlay_visible {
            match k.code {
                KeyCode::Char('?') | KeyCode::Esc => {
                    self.log_overlay_visible = false;
                }
                _ => {}
            }
            return Ok(false);
        }

        // Precedence layer 2: confirm modal.
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
            return Ok(false);
        }

        // Precedence layer 2 (continued): Spans popover (session / kind).
        if let Some(i) = self.focused_column {
            let col_id = self.workspace.columns[i].id.clone();
            let popover = self
                .spans_state
                .get(&col_id)
                .and_then(|s| s.popover);
            if let Some(pk) = popover {
                if self.handle_spans_popover_key(i, &col_id, pk, k) {
                    return Ok(false);
                }
            }
        }

        // Precedence layer 1: text-input mode (Spans column search).
        if let Some(i) = self.focused_column {
            let col_id = self.workspace.columns[i].id.clone();
            let st = self.workspace.columns[i].scenario_type;
            if st == ScenarioType::Spans {
                let active = self
                    .spans_state
                    .get(&col_id)
                    .map(|s| s.search_active)
                    .unwrap_or(false);
                if active {
                    // Esc exits search-input mode.
                    if matches!(k.code, KeyCode::Esc) {
                        if let Some(s) = self.spans_state.get_mut(&col_id) {
                            s.search_active = false;
                        }
                        return Ok(false);
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
                                // Server-side search with 300 ms debounce.
                                self.kick_search_debounce(&col_id, query);
                            }
                            return Ok(false);
                        }
                    }
                }
            }
        }

        // Precedence layer 3: widget-local (Context Growth Widget focused).
        if self.widget_focused {
            match k.code {
                KeyCode::Left => {
                    self.widget_move_cursor(-1);
                    return Ok(false);
                }
                KeyCode::Right => {
                    self.widget_move_cursor(1);
                    return Ok(false);
                }
                KeyCode::Enter => {
                    self.widget_select_current();
                    return Ok(false);
                }
                KeyCode::Esc => {
                    self.widget_focused = false;
                    return Ok(false);
                }
                // Any other key falls through to the global layer.
                _ => {}
            }
        }

        // Precedence layer 4: column scenario keys.
        if let Some(i) = self.focused_column {
            if self.scenario_handle_key(i, k) {
                return Ok(false);
            }
        }

        // Precedence layer 5: global / workspace.
        match (k.code, k.modifiers) {
            (KeyCode::Char('q'), m) if !m.contains(KeyModifiers::SHIFT) => {
                return Ok(true);
            }
            (KeyCode::Char('c'), m) if m.contains(KeyModifiers::CONTROL) => {
                return Ok(true);
            }
            // Phase 2: `c` toggles the Context Growth Widget visibility.
            (KeyCode::Char('c'), m)
                if !m.contains(KeyModifiers::CONTROL) && !m.contains(KeyModifiers::ALT) =>
            {
                self.toggle_context_widget();
            }
            // Phase 2: Alt+↑/↓ resize widget by 1 row; +Shift by 5 rows.
            (KeyCode::Up, m) if m.contains(KeyModifiers::ALT) => {
                let step = if m.contains(KeyModifiers::SHIFT) { 5 } else { 1 };
                self.adjust_widget_height(step);
            }
            (KeyCode::Down, m) if m.contains(KeyModifiers::ALT) => {
                let step = if m.contains(KeyModifiers::SHIFT) { 5 } else { 1 };
                self.adjust_widget_height(-step);
            }
            (KeyCode::Char('?'), _) => {
                self.log_overlay_visible = true;
            }
            (KeyCode::Char('M'), _) => {
                self.toggle_mouse();
            }
            (KeyCode::Tab, _) => self.cycle_focus(1),
            (KeyCode::BackTab, _) => self.cycle_focus(-1),
            (KeyCode::Char('a'), _) => self.append_column(),
            (KeyCode::Char('x'), _) => self.remove_focused_column(),
            _ => {}
        }
        Ok(false)
    }

    /// Dispatch a key to the focused column's scenario. Returns `true` if
    /// the key was consumed.
    fn scenario_handle_key(&mut self, col_idx: usize, k: crossterm::event::KeyEvent) -> bool {
        let st = self.workspace.columns[col_idx].scenario_type;
        let col_id = self.workspace.columns[col_idx].id.clone();
        match st {
            ScenarioType::LiveSessions => self.live_sessions_key(col_idx, &col_id, k),
            ScenarioType::Spans => self.spans_key(col_idx, &col_id, k),
            ScenarioType::ToolDetail => self.tool_detail_key(&col_id, k),
            ScenarioType::ChatDetail => self.chat_detail_key(&col_id, k),
            ScenarioType::FileTouches => self.file_touches_key(&col_id, k),
            _ => false,
        }
    }

    fn live_sessions_key(
        &mut self,
        col_idx: usize,
        col_id: &str,
        k: crossterm::event::KeyEvent,
    ) -> bool {
        let sessions = self.cached_sessions();
        let max = sessions.len();
        let state = self.live_sessions_state.entry(col_id.to_string()).or_default();
        match k.code {
            KeyCode::Down => {
                state.move_cursor(1, max);
                true
            }
            KeyCode::Up => {
                state.move_cursor(-1, max);
                true
            }
            KeyCode::Home => {
                state.jump_top();
                true
            }
            KeyCode::End => {
                state.jump_bottom(max);
                true
            }
            KeyCode::Enter => {
                if let Some(s) = sessions.get(state.cursor) {
                    let cid = s.conversation_id.clone();
                    propagate_session(&mut self.workspace.columns, &cid, col_idx);
                    let _ = persist::save(&self.workspace);
                }
                true
            }
            KeyCode::Char('d') | KeyCode::Delete => {
                if let Some(s) = sessions.get(state.cursor) {
                    let (title, prompt) = delete_prompt(&s.conversation_id);
                    self.confirm_modal.open(title, prompt);
                    self.pending_delete = Some(s.conversation_id.clone());
                }
                true
            }
            _ => false,
        }
    }

    fn spans_key(
        &mut self,
        col_idx: usize,
        col_id: &str,
        k: crossterm::event::KeyEvent,
    ) -> bool {
        let tree = self.cached_session_tree(col_idx);
        let flat = tree.flatten_visible(&self.spans_state.get(col_id).map(|s| s.user_collapsed.clone()).unwrap_or_default());
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
            KeyCode::Char('/') => {
                state.search_active = true;
                true
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
                    .and_then(|cid| sessions.iter().position(|x| &x.conversation_id == cid))
                    .unwrap_or(0);
                if let Some(s) = self.spans_state.get_mut(col_id) {
                    s.popover = Some(SpansPopover::Session);
                    let safe_cursor = cur.min(n_sessions.saturating_sub(1));
                    s.session_picker.open(safe_cursor);
                }
                true
            }
            KeyCode::Char('k') => {
                if let Some(s) = self.spans_state.get_mut(col_id) {
                    s.popover = Some(SpansPopover::Kind);
                    s.kind_picker.open(0);
                }
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
        let cache = self.cache.clone();
        let api = self.api.clone();
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
        let api = self.api.clone();
        let cache = self.cache.clone();
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
        let api = self.api.clone();
        swr_read::<crate::tui::model::ListSessionsResponse, _, _>(
            &self.cache,
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
        let api = self.api.clone();
        let cid_s = cid.to_string();
        swr_read::<crate::tui::model::ListSessionContextsResponse, _, _>(
            &self.cache,
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

    /// `Enter` on the focused widget bar → emit a `clicked_chat` signal into
    /// every Spans column (`Context widget bar click selects chat in Spans
    /// column`). The signal is consumed in the tick path via `spans_pick`.
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
        let signal = (node.trace_id.clone(), node.span_id.clone());
        for c in self.workspace.columns.iter() {
            if c.scenario_type == ScenarioType::Spans {
                let st = self.spans_state.entry(c.id.clone()).or_default();
                st.clicked_chat = Some(signal.clone());
            }
        }
        // Apply immediately as well so the routing is observable without
        // waiting for the next tick.
        self.consume_widget_clicks();
    }

    /// Consume any pending `clicked_chat` signals on Spans columns by routing
    /// them through the shared `spans_pick` selection path, then clear them.
    fn consume_widget_clicks(&mut self) {
        let cols: Vec<(usize, String)> = self
            .workspace
            .columns
            .iter()
            .enumerate()
            .filter(|(_, c)| c.scenario_type == ScenarioType::Spans)
            .map(|(i, c)| (i, c.id.clone()))
            .collect();
        for (idx, col_id) in cols {
            let clicked = self
                .spans_state
                .get(&col_id)
                .and_then(|s| s.clicked_chat.clone());
            let Some((_tid, sid)) = clicked else {
                continue;
            };
            // Move the row cursor to the clicked chat span when it is visible.
            let tree = self.cached_session_tree(idx);
            let collapsed = self
                .spans_state
                .get(&col_id)
                .map(|s| s.user_collapsed.clone())
                .unwrap_or_default();
            let flat = tree.flatten_visible(&collapsed);
            if let Some(pos) = flat.iter().position(|id| id == &sid) {
                if let Some(s) = self.spans_state.get_mut(&col_id) {
                    s.cursor = pos;
                }
            }
            self.spans_pick(idx, &sid);
            if let Some(s) = self.spans_state.get_mut(&col_id) {
                s.clicked_chat = None;
            }
        }
    }

    /// Toggle the Context Growth Widget visibility (`c` global key). Drops
    /// widget focus when hiding.
    fn toggle_context_widget(&mut self) {
        self.workspace.context_widget_visible = !self.workspace.context_widget_visible;
        if !self.workspace.context_widget_visible {
            self.widget_focused = false;
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
        let api = self.api.clone();
        swr_read::<crate::tui::model::ListTracesResponse, _, _>(
            &self.cache,
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
    fn cached_span_detail(
        &self,
        trace_id: &str,
        span_id: &str,
    ) -> Option<crate::tui::model::SpanDetail> {
        let api = self.api.clone();
        let tid = trace_id.to_string();
        let sid = span_id.to_string();
        swr_read::<crate::tui::model::SpanDetail, _, _>(
            &self.cache,
            qkey(["span", trace_id, span_id]),
            std::time::Duration::from_secs(30),
            FetchPolicy::Swr,
            move || async move {
                let r = api.get_span(&tid, &sid).await?;
                Ok::<Value, anyhow::Error>(serde_json::to_value(r)?)
            },
        )
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
            &self.cache,
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
            self.focused_column = None;
            self.widget_focused = false;
            return;
        }
        let cur = if self.widget_focused {
            n
        } else {
            self.focused_column.unwrap_or(0).min(slots - 1)
        };
        let next = ((cur as i32 + dir).rem_euclid(slots as i32)) as usize;
        if widget_in_cycle && next == n {
            self.widget_focused = true;
            if self.context_widget.bar_cursor.is_none() {
                self.context_widget.bar_cursor = Some(0);
            }
            self.publish_widget_hover();
        } else {
            self.widget_focused = false;
            self.focused_column = Some(next);
        }
    }

    fn append_column(&mut self) {
        let all = ScenarioType::all();
        let st = all[self.add_column_cursor % all.len()];
        self.add_column_cursor = (self.add_column_cursor + 1) % all.len();
        self.workspace.add_column(st);
        if self.focused_column.is_none() {
            self.focused_column = Some(self.workspace.columns.len() - 1);
        }
        let _ = persist::save(&self.workspace);
    }

    fn remove_focused_column(&mut self) {
        if let Some(i) = self.focused_column {
            self.workspace.remove_column(i);
            if self.workspace.columns.is_empty() {
                self.focused_column = None;
            } else if i >= self.workspace.columns.len() {
                self.focused_column = Some(self.workspace.columns.len() - 1);
            }
            let _ = persist::save(&self.workspace);
        }
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

    pub fn draw(&self, frame: &mut ratatui::Frame<'_>) {
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
        self.draw_workspace(frame, chunks[1]);
        self.draw_context_widget(frame, chunks[2]);
        if self.log_overlay_visible {
            let lines = self.log_buffer.snapshot();
            frame.render_widget(LogOverlay { lines }, area);
        }
        if self.confirm_modal.open {
            let view = ConfirmModalView {
                state: &self.confirm_modal,
            };
            frame.render_widget(view, area);
        }
    }

    /// Paint the Context Growth Widget strip (or its collapsed bar) at the
    /// bottom of the workspace.
    fn draw_context_widget(&self, frame: &mut ratatui::Frame<'_>, area: Rect) {
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

    fn draw_top_bar(&self, frame: &mut ratatui::Frame<'_>, area: Rect) {
        // status dot in column 0, then text title + hints.
        if area.width < 4 {
            return;
        }
        let dot_area = Rect::new(area.x, area.y, 1, 1);
        let status = StatusDot::new(self.status);
        let title = status.title().to_string();
        frame.render_widget(status, dot_area);

        let next_st = ScenarioType::all()[self.add_column_cursor];
        let hints = format!(
            " ghcp-mon attach │ {title} │ a:add {add} │ x:rm │ Tab:focus │ M:mouse({mouse}) │ ?:logs │ q:quit",
            add = next_st.default_title(),
            mouse = if self.mouse_enabled { "on" } else { "off" },
        );
        let p = Paragraph::new(Span::styled(hints, Style::default().fg(Color::White)));
        let rest = Rect::new(area.x + 2, area.y, area.width - 2, 1);
        frame.render_widget(p, rest);
    }

    fn draw_workspace(&self, frame: &mut ratatui::Frame<'_>, area: Rect) {
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

        for (i, col) in self.workspace.columns.iter().enumerate() {
            let rect = cols[i];
            let focused = self.focused_column == Some(i);
            let mut block = Block::default()
                .borders(Borders::ALL)
                .title(format!(" {} ", col.title));
            if focused {
                block = block.border_style(Style::default().fg(Color::Cyan));
            }
            let inner = block.inner(rect);
            frame.render_widget(block, rect);
            if inner.width < 3 {
                // truncated label
                let buf: &mut Buffer = frame.buffer_mut();
                let span = Span::styled("…", Style::default().fg(Color::DarkGray));
                buf.set_span(inner.x, inner.y, &span, inner.width);
                continue;
            }
            // Dispatch to per-scenario renderer.
            let cfg = col.config.clone();
            let st = col.scenario_type;
            let buf: &mut Buffer = frame.buffer_mut();
            match st {
                ScenarioType::LiveSessions => self.draw_live_sessions(inner, buf, &col.id),
                ScenarioType::Spans => self.draw_spans(inner, buf, i, &col.id),
                ScenarioType::ToolDetail => {
                    self.draw_tool_detail(inner, buf, &col.id, &cfg, focused)
                }
                ScenarioType::ChatDetail => {
                    self.draw_chat_detail(inner, buf, &col.id, &cfg, focused)
                }
                ScenarioType::FileTouches => {
                    self.draw_file_touches(inner, buf, &col.id, &cfg, focused)
                }
                _ => render_placeholder(inner, buf, st, &cfg),
            }
        }
    }

    fn draw_live_sessions(&self, area: Rect, buf: &mut Buffer, col_id: &str) {
        let sessions = self.cached_sessions();
        let default = LiveSessionsState::default();
        let state: &LiveSessionsState =
            self.live_sessions_state.get(col_id).unwrap_or(&default);
        if sessions.is_empty() {
            let line = Line::from(Span::styled(
                "no sessions yet — replay a fixture",
                Style::default()
                    .fg(Color::DarkGray)
                    .add_modifier(Modifier::ITALIC),
            ));
            Paragraph::new(line).render(area, buf);
            return;
        }
        let mut lines: Vec<Line<'static>> = Vec::with_capacity(sessions.len());
        for (i, s) in sessions.iter().enumerate() {
            let txt = render_row(s);
            let style = if i == state.cursor {
                Style::default()
                    .bg(Color::Cyan)
                    .fg(Color::Black)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };
            lines.push(Line::from(Span::styled(txt, style)));
        }
        Paragraph::new(lines).render(area, buf);
    }

    /// Render a `ToolDetail` column. Resolves the configured span selection +
    /// search query and the cached span detail, then delegates to the
    /// tool-detail scenario renderer (which mutates per-column state through
    /// the `RefCell`).
    fn draw_tool_detail(
        &self,
        area: Rect,
        buf: &mut Buffer,
        col_id: &str,
        cfg: &crate::tui::workspace::ColumnConfig,
        focused: bool,
    ) {
        let trace_id = cfg.get("selected_trace_id").and_then(|v| v.as_str());
        let span_id = cfg.get("selected_span_id").and_then(|v| v.as_str());
        let search_query = cfg.get("search_query").and_then(|v| v.as_str());
        let selection = match (trace_id, span_id) {
            (Some(t), Some(s)) => Some((t, s)),
            _ => None,
        };
        let detail = selection.and_then(|(t, s)| self.cached_span_detail(t, s));

        let mut map = self.tool_detail_state.borrow_mut();
        let state = map.entry(col_id.to_string()).or_default();
        crate::tui::scenarios::tool_detail::render(
            area,
            buf,
            state,
            selection,
            search_query,
            detail.as_ref(),
            focused,
        );
    }

    /// Dispatch a key to a `ToolDetail` column's scenario state.
    fn tool_detail_key(&mut self, col_id: &str, k: crossterm::event::KeyEvent) -> bool {
        let mut map = self.tool_detail_state.borrow_mut();
        let state = map.entry(col_id.to_string()).or_default();
        crate::tui::scenarios::tool_detail::handle_key(k, state)
    }

    /// Render a `ChatDetail` column. Resolves the selection, mode, search
    /// query, tool-call hint, and the prior chat-span baseline (DELTA), then
    /// delegates to the chat-detail scenario renderer.
    fn draw_chat_detail(
        &self,
        area: Rect,
        buf: &mut Buffer,
        col_id: &str,
        cfg: &crate::tui::workspace::ColumnConfig,
        focused: bool,
    ) {
        use crate::tui::scenarios::chat_detail::tree::ChatMode;
        let trace_id = cfg.get("selected_trace_id").and_then(|v| v.as_str());
        let span_id = cfg.get("selected_span_id").and_then(|v| v.as_str());
        let selected_tool_call_id =
            cfg.get("selected_tool_call_id").and_then(|v| v.as_str());
        let search_query = cfg.get("search_query").and_then(|v| v.as_str());
        let selection = match (trace_id, span_id) {
            (Some(t), Some(s)) => Some((t, s)),
            _ => None,
        };
        let detail = selection.and_then(|(t, s)| self.cached_span_detail(t, s));

        // Resolve mode from per-column override or from config.
        let cfg_mode = cfg.get("chat_mode").and_then(|v| v.as_str());
        let mut mode_map = self.chat_detail_mode.borrow_mut();
        let mode = *mode_map
            .entry(col_id.to_string())
            .or_insert_with(|| ChatMode::from_config_str(cfg_mode));

        // DELTA prior-chat-span lookup walks the COMPLETE cached
        // `session-span-tree` (per the cache contract). Conversation id
        // comes from the current span's projection.
        let prior_attrs: Option<Value> = match (&detail, mode) {
            (Some(d), ChatMode::Delta) => {
                let cid = d
                    .projection
                    .chat_turn
                    .as_ref()
                    .and_then(|c| c.conversation_id.clone());
                let cid = match cid {
                    Some(c) => c,
                    None => return self.finish_chat_detail_render(
                        area, buf, col_id, selection, search_query,
                        selected_tool_call_id, mode, detail.as_ref(), None, focused,
                    ),
                };
                let tree = self.cached_session_span_tree_by_cid(&cid);
                let prior = tree.find_prior_chat(
                    d.span.span_pk,
                    d.span.end_unix_ns,
                    d.span.start_unix_ns,
                );
                prior.and_then(|node| {
                    self.cached_span_detail(&node.trace_id, &node.span_id)
                        .and_then(|sd| sd.span.attributes)
                })
            }
            _ => None,
        };

        self.finish_chat_detail_render(
            area, buf, col_id, selection, search_query,
            selected_tool_call_id, mode, detail.as_ref(), prior_attrs.as_ref(), focused,
        );
    }

    #[allow(clippy::too_many_arguments)]
    fn finish_chat_detail_render(
        &self,
        area: Rect,
        buf: &mut Buffer,
        col_id: &str,
        selection: Option<(&str, &str)>,
        search_query: Option<&str>,
        selected_tool_call_id: Option<&str>,
        mode: crate::tui::scenarios::chat_detail::tree::ChatMode,
        detail: Option<&crate::tui::model::SpanDetail>,
        prior_attrs: Option<&Value>,
        focused: bool,
    ) {
        let mut map = self.chat_detail_state.borrow_mut();
        let state = map.entry(col_id.to_string()).or_default();
        crate::tui::scenarios::chat_detail::render(
            area,
            buf,
            state,
            selection,
            search_query,
            selected_tool_call_id,
            mode,
            detail,
            prior_attrs,
            focused,
        );
    }

    /// Dispatch a key to a `ChatDetail` column.
    fn chat_detail_key(&mut self, col_id: &str, k: crossterm::event::KeyEvent) -> bool {
        let mut map = self.chat_detail_state.borrow_mut();
        let state = map.entry(col_id.to_string()).or_default();
        let mut mode_map = self.chat_detail_mode.borrow_mut();
        let mode_entry = mode_map.entry(col_id.to_string()).or_insert(
            crate::tui::scenarios::chat_detail::tree::ChatMode::Delta,
        );
        crate::tui::scenarios::chat_detail::handle_key(k, state, mode_entry)
    }

    /// Render a `FileTouches` column. Resolves the configured session, walks
    /// the cached session span tree for file-touching tool spans (fetching each
    /// span's detail through the shared `["span", ...]` cache), and delegates to
    /// the file-touches scenario renderer.
    fn draw_file_touches(
        &self,
        area: Rect,
        buf: &mut Buffer,
        col_id: &str,
        cfg: &crate::tui::workspace::ColumnConfig,
        focused: bool,
    ) {
        let session = cfg.get("session").and_then(|v| v.as_str());
        let (cache_loaded, touches) = match session {
            Some(s) => {
                let tree = self.cached_session_span_tree_by_cid(s);
                // Distinguish "loading" from "no touches": the tree fetch is
                // complete once the cache key holds a value.
                let loaded = self.cache.peek(&qkey(["session-span-tree", s])).value.is_some();
                let touches = crate::tui::scenarios::file_touches::walk::extract_touches(
                    &tree,
                    |t, sp| self.cached_span_detail(t, sp),
                );
                (loaded, touches)
            }
            None => (false, Vec::new()),
        };

        let mut map = self.file_touches_state.borrow_mut();
        let state = map.entry(col_id.to_string()).or_default();
        crate::tui::scenarios::file_touches::render(
            area,
            buf,
            state,
            session,
            cache_loaded,
            &touches,
            focused,
        );
    }

    /// Dispatch a key to a `FileTouches` column's scenario state.
    fn file_touches_key(&mut self, col_id: &str, k: crossterm::event::KeyEvent) -> bool {
        let mut map = self.file_touches_state.borrow_mut();
        let state = map.entry(col_id.to_string()).or_default();
        crate::tui::scenarios::file_touches::handle_key(k, state)
    }

    /// Read-or-fetch `["session-span-tree", cid]` keyed by cid directly.
    /// Returns the complete server tree (per the cache contract — DELTA's
    /// prior-chat-span walk MUST read this, not Spans' reveal-filtered view).
    fn cached_session_span_tree_by_cid(
        &self,
        cid: &str,
    ) -> Vec<crate::tui::model::SpanNode> {
        let api = self.api.clone();
        let cid_s = cid.to_string();
        swr_read::<SessionSpanTreeResponse, _, _>(
            &self.cache,
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

    fn draw_spans(&self, area: Rect, buf: &mut Buffer, col_idx: usize, col_id: &str) {
        let cfg = &self.workspace.columns[col_idx].config;
        let session = cfg.get("session").and_then(|v| v.as_str()).map(str::to_string);
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

        // Row 2: kind / search / follow / collapse hints.
        let default_state = SpansState::default();
        let st: &SpansState = self.spans_state.get(col_id).unwrap_or(&default_state);
        let follow = if st.follow_mode { "[x] follow" } else { "[ ] follow" };
        let search_label = if st.search_active {
            format!("/ {}_", st.search.text())
        } else if !st.search.text().is_empty() {
            format!("/ {}", st.search.text())
        } else {
            "/  ".to_string()
        };
        let kind_filter: Option<String> = cfg
            .get("kind_filter")
            .and_then(|v| v.as_str())
            .map(str::to_string);
        let kf_label = kind_filter
            .as_deref()
            .map(|s| format!("k:{s}"))
            .unwrap_or_else(|| "k:kind".to_string());
        let hint = format!(
            "{kf}  {search}  {follow}  +/-:expand/collapse  s:session",
            kf = kf_label,
            search = search_label,
        );
        Paragraph::new(Span::styled(hint, Style::default().fg(Color::DarkGray)))
            .render(h_bot, buf);

        // No-session mode: render traces list.
        let Some(session) = session else {
            self.draw_traces_list(body_total, buf, col_id);
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
            let dots = crate::tui::widgets::rolling_dots::frame(self.anim_tick);
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
        let flat = tree.flatten_visible(&st.user_collapsed);
        let visible_rows = tree_area.height as usize;
        let start = if st.cursor >= visible_rows {
            st.cursor + 1 - visible_rows
        } else {
            0
        };
        let search_text = st.search.text();
        // Resolve search-hit set from the cache (server-side search).
        let hit_set: Option<std::collections::HashSet<String>> = if !search_text.is_empty() {
            self.cached_search_hits(&session, search_text).map(|resp| {
                resp.results.into_iter().map(|r| r.span_id).collect()
            })
        } else {
            None
        };

        // Pre-compute per-parent report_intent titles (latest direct child with
        // tool_name == "report_intent" → intent string).
        let report_titles = self.compute_report_intent_titles(&tree);

        for (i_visible, flat_idx) in (start..flat.len().min(start + visible_rows)).enumerate() {
            let row_id = &flat[flat_idx];
            let (node, depth) = match tree.find_with_depth(row_id) {
                Some((n, d)) => (Some(n), d),
                None => (None, 0),
            };
            let Some(node) = node else { continue };
            let row_y = tree_area.y + i_visible as u16;
            let focused = flat_idx == st.cursor;
            let mut x = tree_area.x;
            let indent: u16 = (depth as u16) * 2;
            x += indent;
            // Determine row-wide background style based on search hit / miss.
            let (row_bg, row_dim) = match &hit_set {
                Some(set) if set.contains(&node.span_id) => (Some(Color::Yellow), false),
                Some(_) => (None, true),
                None => (None, false),
            };
            // Collapse glyph
            let collapsed = st.user_collapsed.contains(row_id);
            let glyph = if node.children.is_empty() {
                " "
            } else if collapsed {
                "▸"
            } else {
                "▾"
            };
            let glyph_style = if focused {
                Style::default().bg(Color::Cyan).fg(Color::Black)
            } else if let Some(bg) = row_bg {
                Style::default().bg(bg).fg(Color::Black).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };
            buf.set_span(x, row_y, &Span::styled(glyph, glyph_style), 1);
            x += 2;
            // Kind badge
            let label = kind_label(node.kind_class);
            let badge_w = (label.chars().count() as u16 + 2).min(10);
            if x + badge_w < tree_area.x + tree_area.width {
                let badge = KindBadge::new(node.kind_class).with_seed(node.name.clone());
                badge.render(Rect::new(x, row_y, badge_w, 1), buf);
                x += badge_w + 1;
            }
            // Placeholder rolling dots
            if node.ingestion_state == "placeholder"
                && x + 3 < tree_area.x + tree_area.width
            {
                let dots = crate::tui::widgets::rolling_dots::frame(self.anim_tick);
                buf.set_span(
                    x,
                    row_y,
                    &Span::styled(dots.to_string(), Style::default().fg(Color::Yellow)),
                    3,
                );
                x += 4;
            }
            // Build chip strings + optional description for this row.
            let (chips, description) = self.compute_row_chips(node);
            // Approximate chip width to reserve before truncating the name.
            // Each chip costs `text.len() + 2` (padding) + 1 gap.
            let chip_reserve: usize = chips
                .iter()
                .map(|(s, _)| s.chars().count() + 3)
                .sum::<usize>()
                .min(40);
            let desc_reserve = description
                .as_deref()
                .map(|s| s.chars().count() + 1)
                .unwrap_or(0);
            // Report-intent title (white text appended at end of parent row).
            let report_title = report_titles.get(&node.span_id).cloned();
            let title_reserve = report_title
                .as_deref()
                .map(|s| s.chars().count() + 2)
                .unwrap_or(0);

            // Name (truncated to leave room for chips + description + title).
            let total_avail =
                (tree_area.x + tree_area.width).saturating_sub(x) as usize;
            let name_budget = total_avail
                .saturating_sub(chip_reserve + desc_reserve + title_reserve);
            let mut name = node.name.clone();
            let cc = name.chars().count();
            if cc > name_budget {
                name = name
                    .chars()
                    .take(name_budget.saturating_sub(1))
                    .collect::<String>();
                if !name.is_empty() {
                    name.push('…');
                }
            }
            let mut name_style = if focused {
                Style::default()
                    .bg(Color::Cyan)
                    .fg(Color::Black)
                    .add_modifier(Modifier::BOLD)
            } else if row_bg.is_some() {
                Style::default()
                    .bg(Color::Yellow)
                    .fg(Color::Black)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };
            if row_dim {
                name_style = name_style.fg(Color::DarkGray).add_modifier(Modifier::DIM);
            }
            if let Some(kf) = &kind_filter {
                let cur = format!("{:?}", node.kind_class).to_lowercase();
                if cur != kf.to_lowercase() {
                    name_style = name_style.add_modifier(Modifier::DIM);
                }
            }
            if x < tree_area.x + tree_area.width {
                let name_w = (name.chars().count() as u16).min(
                    (tree_area.x + tree_area.width).saturating_sub(x),
                );
                buf.set_span(x, row_y, &Span::styled(name, name_style), name_w);
                x += name_w;
            }
            // Chips
            for (text, color) in &chips {
                if x + 1 >= tree_area.x + tree_area.width {
                    // Out of room — append overflow marker if possible.
                    break;
                }
                x += 1;
                let chip_text = format!(" {text} ");
                let chip_w = (chip_text.chars().count() as u16)
                    .min((tree_area.x + tree_area.width).saturating_sub(x));
                let chip_style = Style::default()
                    .bg(*color)
                    .fg(Color::Black)
                    .add_modifier(Modifier::BOLD);
                buf.set_span(x, row_y, &Span::styled(chip_text, chip_style), chip_w);
                x += chip_w;
            }
            // Tool description label (no chip styling — white text, 1-cell
            // left padding) per `Spans tool description inline label`.
            if let Some(desc) = description {
                if x + 1 < tree_area.x + tree_area.width {
                    x += 1;
                    let w = (desc.chars().count() as u16)
                        .min((tree_area.x + tree_area.width).saturating_sub(x));
                    buf.set_span(
                        x,
                        row_y,
                        &Span::styled(desc, Style::default().fg(Color::White)),
                        w,
                    );
                    x += w;
                }
            }
            // Report-intent title (no chip styling — white text).
            if let Some(title) = report_title {
                if x + 1 < tree_area.x + tree_area.width {
                    x += 1;
                    let w = (title.chars().count() as u16)
                        .min((tree_area.x + tree_area.width).saturating_sub(x));
                    buf.set_span(
                        x,
                        row_y,
                        &Span::styled(title, Style::default().fg(Color::White)),
                        w,
                    );
                }
            }
        }

        // Bottom detail inspector pane.
        if detail_h >= 3 {
            self.draw_span_detail_pane(detail_area, buf, &tree, &flat, st);
        }

        // Popover overlay (drawn over the body).
        self.draw_spans_popover(body_total, buf, col_id);
    }

    /// Render the no-session traces list. Implements `Traces list dims rows
    /// below kind filter`.
    fn draw_traces_list(&self, area: Rect, buf: &mut Buffer, col_id: &str) {
        let traces = self.cached_traces();
        let default = SpansState::default();
        let st: &SpansState = self.spans_state.get(col_id).unwrap_or(&default);
        if traces.is_empty() {
            let dots = crate::tui::widgets::rolling_dots::frame(self.anim_tick);
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
        &self,
        area: Rect,
        buf: &mut Buffer,
        tree: &[crate::tui::model::SpanNode],
        flat: &[String],
        st: &SpansState,
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
        let Some(focused_id) = flat.get(st.cursor) else {
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
        &self,
        node: &crate::tui::model::SpanNode,
    ) -> (Vec<(String, Color)>, Option<String>) {
        let mut out: Vec<(String, Color)> = Vec::new();
        // Only execute_tool spans currently get chips per the LLR family.
        if !matches!(node.kind_class, KindClass::ExecuteTool) {
            return (out, None);
        }
        let Some(tool_call) = &node.projection.tool_call else {
            return (out, None);
        };
        let tool_name = tool_call.tool_name.clone().unwrap_or_default();
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
        // Diff-stat badges
        let kind_opt = crate::tui::vendor::copilot::tool_name_mapping(&tool_name);
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
        &self,
        tree: &[crate::tui::model::SpanNode],
    ) -> std::collections::HashMap<String, String> {
        let mut out: std::collections::HashMap<String, String> = std::collections::HashMap::new();
        fn walk(
            node: &crate::tui::model::SpanNode,
            app: &App,
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

    /// Popover overlay for the `s` (session) and `k` (kind) keys.
    fn draw_spans_popover(&self, area: Rect, buf: &mut Buffer, col_id: &str) {
        let Some(st) = self.spans_state.get(col_id) else {
            return;
        };
        let Some(which) = st.popover else { return };
        use crate::tui::widgets::select::SelectPopover;
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
pub async fn event_loop(
    terminal: &mut DefaultTerminal,
    app: &mut App,
    mut rx: mpsc::Receiver<AppEvent>,
) -> Result<()> {
    loop {
        // Block until at least one event arrives, then drain everything
        // else before drawing.
        let first = match rx.recv().await {
            Some(e) => e,
            None => break,
        };
        let mut quit = app.handle(first)?;
        while !quit {
            match rx.try_recv() {
                Ok(e) => quit = app.handle(e)?,
                Err(mpsc::error::TryRecvError::Empty) => break,
                Err(mpsc::error::TryRecvError::Disconnected) => {
                    quit = true;
                    break;
                }
            }
        }
        terminal.draw(|f| app.draw(f))?;
        if quit {
            break;
        }
    }
    Ok(())
}

/// Spawn all the background event sources (ws coalescer, crossterm reader,
/// animation tick). The caller wires the resulting receiver into
/// [`event_loop`].
pub fn spawn_event_sources(
    ws: &WsBus,
) -> (mpsc::Sender<AppEvent>, mpsc::Receiver<AppEvent>) {
    let (tx, rx) = crate::tui::event::channel();
    spawn_ws_coalescer(ws.subscribe(), tx.clone());
    spawn_crossterm_reader(tx.clone());
    spawn_tick(tx.clone());
    (tx, rx)
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
    fn append_column_cycles_scenario_types() {
        let mut app = make_app();
        app.workspace.columns.clear();
        app.focused_column = None;
        let n0 = app.workspace.columns.len();
        app.append_column();
        app.append_column();
        assert_eq!(app.workspace.columns.len(), n0 + 2);
        assert_ne!(
            app.workspace.columns[n0].scenario_type,
            app.workspace.columns[n0 + 1].scenario_type
        );
    }

    #[test]
    fn empty_workspace_renders_hint() {
        use ratatui::Terminal;
        use ratatui::backend::TestBackend;
        let mut app = make_app();
        app.workspace.columns.clear();
        app.focused_column = None;
        let backend = TestBackend::new(80, 12);
        let mut term = Terminal::new(backend).unwrap();
        term.draw(|f| app.draw(f)).unwrap();
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

    /// The drain-then-draw rule: many WS ticks should not trigger one draw
    /// per event. We can't observe draws directly here, but we *can* observe
    /// that `handle` does not return quit and that the event queue can be
    /// emptied in one go.
    #[test]
    fn handle_processes_many_ws_ticks() {
        let mut app = make_app();
        for _ in 0..50 {
            let r = app
                .handle(AppEvent::WsTick {
                    dirty_prefixes: vec![],
                    envelopes: vec![],
                })
                .unwrap();
            assert!(!r);
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
            name: id.into(),
            kind_class: kind,
            ingestion_state: "complete".into(),
            start_unix_ns: Some(end_ns),
            end_unix_ns: Some(end_ns),
            projection,
            children,
        }
    }

    fn seed_session_tree(app: &App, cid: &str, tree: Vec<SpanNode>) {
        let resp = SessionSpanTreeResponse {
            conversation_id: cid.into(),
            tree,
        };
        app.cache.put(FetchedRecord {
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
        app.cache.put(FetchedRecord {
            key: crate::tui::cache::qkey(["span", trace_id, span_id]),
            generation: 1,
            value: serde_json::to_value(detail).unwrap(),
            stale_after: std::time::Duration::from_secs(60),
        });
    }

    fn seed_sessions(app: &App, sessions: Vec<SessionSummary>) {
        let r = ListSessionsResponse { sessions };
        app.cache.put(FetchedRecord {
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
        app.cache.put(FetchedRecord {
            key: crate::tui::cache::qkey(["search-spans", session, q]),
            generation: 1,
            value: serde_json::to_value(resp).unwrap(),
            stale_after: std::time::Duration::from_secs(60),
        });
    }

    fn seed_traces(app: &App, traces: Vec<TraceSummary>) {
        let r = ListTracesResponse { traces };
        app.cache.put(FetchedRecord {
            key: crate::tui::cache::qkey(["traces"]),
            generation: 1,
            value: serde_json::to_value(r).unwrap(),
            stale_after: std::time::Duration::from_secs(60),
        });
    }

    fn render_buf_text(app: &App, w: u16, h: u16) -> String {
        use ratatui::Terminal;
        use ratatui::backend::TestBackend;
        let backend = TestBackend::new(w, h);
        let mut term = Terminal::new(backend).unwrap();
        term.draw(|f| app.draw(f)).unwrap();
        let buf = term.backend().buffer();
        let mut joined = String::new();
        for y in 0..buf.area.height {
            for x in 0..buf.area.width {
                joined.push_str(buf[(x, y)].symbol());
            }
            joined.push('\n');
        }
        joined
    }

    fn one_spans_column_app() -> App {
        let mut app = make_app();
        app.workspace.columns.clear();
        app.workspace
            .add_column(crate::tui::workspace::ScenarioType::Spans);
        app.focused_column = Some(0);
        app
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
        let text = render_buf_text(&app, 120, 20);
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
        let text = render_buf_text(&app, 120, 20);
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
        let text = render_buf_text(&app, 120, 20);
        assert!(text.contains("DOTHETHING"), "missing intent in:\n{text}");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn follow_mode_advances_cursor_to_latest_tool_span_on_tick() {
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
        // Drive one Tick.
        let _ = app.handle(AppEvent::Tick).unwrap();
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
        let tree = vec![
            mk_span_node("hit-span", KindClass::ExecuteTool, Some("bash"), 100, vec![]),
            mk_span_node("miss-span", KindClass::ExecuteTool, Some("bash"), 200, vec![]),
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
        let text = render_buf_text(&app, 120, 20);
        assert!(text.contains("hit-span"), "missing hit in:\n{text}");
        assert!(text.contains("miss-span"), "missing miss in:\n{text}");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn traces_mode_renders_when_no_session() {
        let app = one_spans_column_app();
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
        let text = render_buf_text(&app, 120, 20);
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
        let text = render_buf_text(&app, 120, 20);
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
        let text = render_buf_text(&app, 120, 20);
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
        app.cache.put(FetchedRecord {
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
        app.widget_focused = true;
        app.toggle_context_widget();
        assert!(!app.workspace.context_widget_visible);
        assert!(!app.widget_focused, "hiding must drop widget focus");
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
        app.focused_column = Some(0);
        app.widget_focused = false;
        // One column + widget = 2 slots. Tab forward lands on the widget.
        app.cycle_focus(1);
        assert!(app.widget_focused, "expected widget focus after column");
        assert_eq!(
            app.context_widget.bar_cursor,
            Some(0),
            "entering widget focus seeds bar cursor"
        );
        // Tab again wraps back to the column.
        app.cycle_focus(1);
        assert!(!app.widget_focused);
        assert_eq!(app.focused_column, Some(0));
    }

    #[test]
    fn cycle_focus_skips_widget_slot_when_hidden() {
        let mut app = one_spans_column_app();
        app.workspace.context_widget_visible = false;
        app.focused_column = Some(0);
        app.cycle_focus(1);
        assert!(!app.widget_focused);
        assert_eq!(app.focused_column, Some(0));
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
        app.widget_focused = true;
        app.context_widget.bar_cursor = Some(1); // chat "b"
        app.widget_select_current();
        // consume_widget_clicks (called inline) routes through spans_pick and
        // clears the signal.
        let col_id = app.workspace.columns[0].id.clone();
        let st = app.spans_state.get(&col_id).unwrap();
        assert!(st.clicked_chat.is_none(), "signal must be consumed");
        // Cursor moved to the flat index of span "b" (index 1).
        assert_eq!(st.cursor, 1);
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
        use ratatui::crossterm::event::{KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};
        KeyEvent {
            code,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        }
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
        app.widget_focused = true;
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
