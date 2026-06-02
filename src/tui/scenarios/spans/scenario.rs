//! `SpansScenario` — the trait-based `Scenario` implementation for the
//! Spans column. Owns one [`SpansState`] (per-column scenario state is
//! one-per-instance because App holds one `Box<dyn Scenario>` per column id).
//!
//! ## Cross-cutting behaviours (all driven by App via the trait):
//!
//! * Layer 1 text-input — `text_input_active` is true while `search_active`
//!   so App's `layer_text_input` routes printable keys here first. The
//!   scenario's `handle_key` runs the search-input branch and returns
//!   `KeyOutcome::pass()` (not consumed) for keys SearchInput rejects,
//!   so they fall through to the next layer as today.
//! * Layer 2 popover — `popover_active` is true while a session/kind
//!   picker is open. App's `layer_modal` dispatches the key here and
//!   always treats the result as consumed (so non-matching keys cannot
//!   quit / cycle focus while a picker is open).
//! * Animation tick — `tick` drains the reveal queue's due entries;
//!   `next_anim_deadline` returns the head of the queue.
//! * Follow-mode advance — `on_ws_batch` (gated on `WsBatchMeta.touches_spans`)
//!   AND `on_cache_changed` both advance any engaged follow-mode column's
//!   cursor to the latest tool span.
//! * Widget bar pick — `pick_span_externally` (NOT part of the Scenario
//!   trait; accessed by App through `as_any_mut().downcast_mut`) lets
//!   the Context Growth Widget reposition this column's cursor and emit
//!   the standard `PropagateSelection` effects.

use std::collections::{HashMap, HashSet};
use std::time::{SystemTime, UNIX_EPOCH};

use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Widget};

use crate::tui::app::DrawOutcome;
use crate::tui::model::{KindClass, SpanNode, SpanTreeExt};
use crate::tui::scenarios::scenario::{Ctx, KeyOutcome, Scenario, WsBatchMeta};
use crate::tui::scenarios::ScenarioEffect;
use crate::tui::widgets::kind_badge::kind_label;
use crate::tui::widgets::select::SelectPopover;
use crate::tui::widgets::spans_tree_row::SpansTreeRow;
use crate::tui::workspace::ColumnConfig;

use super::{
    SelectionPatch, SpansPopover, SpansState, attrs, chips, follow_chat, follow_mode,
    hovered_chat_ancestor, invoke_agent,
};

/// Wall-clock millis since Unix epoch — local helper so `draw` /
/// `tick` don't need it threaded through the trait.
fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Per-column Spans scenario instance. One per Spans column id.
#[derive(Debug, Default)]
pub struct SpansScenario {
    pub state: SpansState,
}

impl SpansScenario {
    pub fn new() -> Self {
        Self::default()
    }

    /// Read-only test/widget accessor.
    pub fn state(&self) -> &SpansState {
        &self.state
    }

    /// Mutable test accessor. Production code MUST go through scenario
    /// methods — this exists for the cfg(test) integration tests that
    /// previously poked `App::spans_state` directly.
    pub fn state_mut(&mut self) -> &mut SpansState {
        &mut self.state
    }

    /// External cursor-pick (Context Growth Widget). Moves the row
    /// cursor to `span_id` if it's visible in the column's flatten,
    /// then emits the standard selection-routing effects via
    /// [`Self::pick_user`].
    ///
    /// Not a Scenario trait method — App reaches into this via
    /// `as_any_mut().downcast_mut::<SpansScenario>()`. Keeping it off
    /// the trait avoids bloating the trait with a Spans-only concept.
    pub fn pick_span_externally(
        &mut self,
        ctx: &mut Ctx<'_>,
        col_idx: usize,
        config: &ColumnConfig,
        span_id: &str,
    ) -> Vec<ScenarioEffect> {
        let tree = self.session_tree(ctx, config);
        let flat = tree.flatten_visible(&self.state.user_collapsed);
        if let Some(pos) = flat.iter().position(|id| id == span_id) {
            self.state.cursor = pos;
        }
        self.pick_user(ctx, col_idx, config, span_id)
    }

    // ---- per-column helpers (private) ----

    /// Fetch the column's session span tree. Returns `Vec::new()` when
    /// no session is configured or the cache is empty.
    fn session_tree(&self, ctx: &Ctx<'_>, config: &ColumnConfig) -> Vec<SpanNode> {
        let Some(cid) = config.get("session").and_then(|v| v.as_str()) else {
            return Vec::new();
        };
        if cid.is_empty() {
            return Vec::new();
        }
        ctx.cached_session_span_tree(cid)
    }

    fn config_session(config: &ColumnConfig) -> Option<String> {
        config
            .get("session")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .map(str::to_string)
    }

    /// Sync session-switch reset bookkeeping (deferred — see scenario
    /// invariants). Not called from any hook today to preserve the
    /// pre-migration behaviour where per-column state survives session
    /// changes; left in place for future wiring.
    #[allow(dead_code)]
    fn sync_session(&mut self, config: &ColumnConfig) {
        let new_session = Self::config_session(config);
        self.state.on_session_switch(new_session.as_deref());
    }

    /// User-initiated selection routing. Idempotent. May toggle
    /// follow-mode (engaging when the picked span IS the latest tool
    /// span, disengaging otherwise) and persists the workspace.
    fn pick_user(
        &mut self,
        ctx: &Ctx<'_>,
        col_idx: usize,
        config: &ColumnConfig,
        picked_span_id: &str,
    ) -> Vec<ScenarioEffect> {
        let tree = self.session_tree(ctx, config);
        let Some(node) = tree.find_by_id(picked_span_id) else {
            return Vec::new();
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
            KindClass::InvokeAgent => invoke_agent::latest_chat_descendant(&tree, picked_span_id)
                .map(|(t, s)| SelectionPatch {
                    trace_id: t,
                    span_id: s,
                    tool_call_id: None,
                }),
            _ => None,
        };

        // Update local state mirror (focused id + follow-mode latch).
        let latest = follow_mode::latest_tool_span(&tree).map(|(_, sid)| sid);
        self.state.focused_span_id = Some(picked_span_id.to_string());
        self.state.follow_mode = latest.as_deref() == Some(picked_span_id);

        // Hover pk publish — emit AFTER selection so consumers observe a
        // hovered pk consistent with the workspace selection (per the
        // ordering note in `ScenarioEffect::SetHoveredChatPk`).
        let flat = tree.flatten_visible(&self.state.user_collapsed);
        let hover_pk = flat
            .get(self.state.cursor)
            .and_then(|id| hovered_chat_ancestor(&tree, id));

        vec![
            ScenarioEffect::PropagateSelection {
                picked_kind,
                picked,
                chat_route,
                origin_col_idx: col_idx,
            },
            ScenarioEffect::SetHoveredChatPk(hover_pk),
            ScenarioEffect::PersistWorkspace,
        ]
    }

    /// Follow-mode advance helper. Distinct from [`Self::pick_user`]: does
    /// NOT toggle follow-mode (stays engaged) and does NOT persist the
    /// workspace (idempotent advance shouldn't dirty disk).
    fn advance_follow_mode_if_engaged(
        &mut self,
        ctx: &Ctx<'_>,
        col_idx: usize,
        config: &ColumnConfig,
    ) -> Vec<ScenarioEffect> {
        if !self.state.follow_mode {
            return Vec::new();
        }
        let tree = self.session_tree(ctx, config);
        let Some((tid, sid)) = follow_mode::latest_tool_span(&tree) else {
            return Vec::new();
        };
        if self.state.focused_span_id.as_deref() == Some(sid.as_str()) {
            return Vec::new();
        }
        let flat = tree.flatten_visible(&self.state.user_collapsed);
        let Some(new_idx) = flat.iter().position(|id| id == &sid) else {
            return Vec::new();
        };
        self.state.cursor = new_idx;
        self.state.focused_span_id = Some(sid.clone());

        let Some(node) = tree.find_by_id(&sid) else {
            return Vec::new();
        };
        let picked = SelectionPatch {
            trace_id: tid,
            span_id: sid.clone(),
            tool_call_id: node
                .projection
                .tool_call
                .as_ref()
                .and_then(|tc| tc.call_id.clone()),
        };
        let chat_route = follow_chat::find_following_chat_span(&tree, &sid).map(|(t, s)| {
            SelectionPatch {
                trace_id: t,
                span_id: s,
                tool_call_id: None,
            }
        });
        vec![ScenarioEffect::PropagateSelection {
            picked_kind: node.kind_class,
            picked,
            chat_route,
            origin_col_idx: col_idx,
        }]
        // No PersistWorkspace, no SetHoveredChatPk — follow-mode advance
        // mirrors the previous tick_follow_mode_advance which did neither.
    }

    /// Compute hover pk for the current cursor. Returns a
    /// `SetHoveredChatPk` effect.
    fn hover_effect(&self, ctx: &Ctx<'_>, config: &ColumnConfig) -> ScenarioEffect {
        let tree = self.session_tree(ctx, config);
        let flat = tree.flatten_visible(&self.state.user_collapsed);
        let pk = flat
            .get(self.state.cursor)
            .and_then(|id| hovered_chat_ancestor(&tree, id));
        ScenarioEffect::SetHoveredChatPk(pk)
    }

    // ---- key dispatch ----

    fn handle_text_input_key(
        &mut self,
        ctx: &mut Ctx<'_>,
        col_idx: usize,
        col_id: &str,
        config: &ColumnConfig,
        k: KeyEvent,
    ) -> KeyOutcome {
        // Esc exits search-input mode; query text is preserved (btop semantics).
        if matches!(k.code, KeyCode::Esc) {
            self.state.search_active = false;
            return KeyOutcome::consumed();
        }
        // Delete exits AND clears the query (btop).
        if matches!(k.code, KeyCode::Delete) {
            self.state.search_active = false;
            self.state.search.clear();
            let _ = self.state.search.take_changed();
            self.state.last_search_emitted.clear();
            self.state.search_hits = None;
            self.state.search_nonce = self.state.search_nonce.wrapping_add(1);
            if let Some(session) = Self::config_session(config) {
                ctx.kick_search_debounce(&session, String::new());
            }
            return KeyOutcome::consumed().with_effects(vec![
                ScenarioEffect::PropagateSearch { query: String::new() },
                ScenarioEffect::PersistWorkspace,
            ]);
        }
        // Enter / Shift+Enter cycle through search matches.
        if matches!(k.code, KeyCode::Enter)
            && !k.modifiers.contains(KeyModifiers::CONTROL)
            && !k.modifiers.contains(KeyModifiers::ALT)
        {
            let forward = !k.modifiers.contains(KeyModifiers::SHIFT);
            let effects = self.jump_to_match(ctx, col_idx, col_id, config, forward);
            return KeyOutcome::consumed().with_effects(effects);
        }
        // SearchInput handles printables / arrows / backspace.
        if self.state.search.handle_key(k) {
            let mut effects: Vec<ScenarioEffect> = Vec::new();
            if self.state.search.take_changed() {
                let t = self.state.search.text().to_string();
                if t != self.state.last_search_emitted {
                    self.state.last_search_emitted = t.clone();
                    effects.push(ScenarioEffect::PropagateSearch { query: t.clone() });
                    effects.push(ScenarioEffect::PersistWorkspace);
                    // Debounced server-side search fetch.
                    if let Some(session) = Self::config_session(config) {
                        self.state.search_nonce = self.state.search_nonce.wrapping_add(1);
                        if t.is_empty() {
                            self.state.search_hits = None;
                        } else {
                            ctx.kick_search_debounce(&session, t);
                        }
                    }
                }
            }
            return KeyOutcome::consumed().with_effects(effects);
        }
        // SearchInput didn't claim — fall through to subsequent layers.
        KeyOutcome::pass()
    }

    fn handle_popover_key(
        &mut self,
        ctx: &Ctx<'_>,
        col_idx: usize,
        which: SpansPopover,
        k: KeyEvent,
    ) -> KeyOutcome {
        match which {
            SpansPopover::Session => {
                let options = ctx.cached_sessions();
                let max = options.len();
                match k.code {
                    KeyCode::Esc => {
                        self.state.session_picker.close();
                        self.state.popover = None;
                        KeyOutcome::consumed()
                    }
                    KeyCode::Up => {
                        self.state.session_picker.move_cursor(-1, max);
                        KeyOutcome::consumed()
                    }
                    KeyCode::Down => {
                        self.state.session_picker.move_cursor(1, max);
                        KeyOutcome::consumed()
                    }
                    KeyCode::Enter => {
                        let cursor = self.state.session_picker.cursor;
                        self.state.session_picker.close();
                        self.state.popover = None;
                        if let Some(opt) = options.get(cursor) {
                            let cid = opt.conversation_id.clone();
                            return KeyOutcome::consumed().with_effects(vec![
                                ScenarioEffect::PropagateSession {
                                    origin_col_idx: col_idx,
                                    cid,
                                },
                                ScenarioEffect::PersistWorkspace,
                            ]);
                        }
                        KeyOutcome::consumed()
                    }
                    _ => KeyOutcome::consumed(), // swallow other keys
                }
            }
            SpansPopover::Kind => {
                let options: &[&str] = &[
                    "chat",
                    "execute_tool",
                    "external_tool",
                    "invoke_agent",
                    "other",
                ];
                let max = options.len();
                match k.code {
                    KeyCode::Esc => {
                        self.state.kind_picker.close();
                        self.state.popover = None;
                        KeyOutcome::consumed()
                    }
                    KeyCode::Up => {
                        self.state.kind_picker.move_cursor(-1, max);
                        KeyOutcome::consumed()
                    }
                    KeyCode::Down => {
                        self.state.kind_picker.move_cursor(1, max);
                        KeyOutcome::consumed()
                    }
                    KeyCode::Enter => {
                        let cursor = self.state.kind_picker.cursor;
                        self.state.kind_picker.close();
                        self.state.popover = None;
                        if let Some(opt) = options.get(cursor) {
                            return KeyOutcome::consumed().with_effects(vec![
                                ScenarioEffect::SetKindFilter {
                                    col_idx,
                                    value: Some((*opt).to_string()),
                                },
                                ScenarioEffect::PersistWorkspace,
                            ]);
                        }
                        KeyOutcome::consumed()
                    }
                    KeyCode::Delete | KeyCode::Backspace => {
                        self.state.kind_picker.close();
                        self.state.popover = None;
                        KeyOutcome::consumed().with_effects(vec![
                            ScenarioEffect::SetKindFilter { col_idx, value: None },
                            ScenarioEffect::PersistWorkspace,
                        ])
                    }
                    _ => KeyOutcome::consumed(),
                }
            }
        }
    }

    fn handle_main_key(
        &mut self,
        ctx: &mut Ctx<'_>,
        col_idx: usize,
        col_id: &str,
        config: &ColumnConfig,
        k: KeyEvent,
    ) -> KeyOutcome {
        // Common (mode-agnostic) keys.
        match k.code {
            KeyCode::Char('/') => {
                self.state.search_active = true;
                return KeyOutcome::consumed();
            }
            KeyCode::Char('s') => {
                let sessions = ctx.cached_sessions();
                let n_sessions = sessions.len();
                let current_cid = config
                    .get("session")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
                let cur = current_cid
                    .as_ref()
                    .and_then(|cid| sessions.iter().position(|x| &x.conversation_id == cid))
                    .unwrap_or(0);
                self.state.popover = Some(SpansPopover::Session);
                let safe_cursor = cur.min(n_sessions.saturating_sub(1));
                self.state.session_picker.open(safe_cursor);
                return KeyOutcome::consumed();
            }
            KeyCode::Char('k') => {
                self.state.popover = Some(SpansPopover::Kind);
                self.state.kind_picker.open(0);
                return KeyOutcome::consumed();
            }
            _ => {}
        }

        let has_session = Self::config_session(config).is_some();
        if has_session {
            self.handle_tree_key(ctx, col_idx, col_id, config, k)
        } else {
            self.handle_traces_key(ctx, k)
        }
    }

    fn handle_traces_key(&mut self, ctx: &Ctx<'_>, k: KeyEvent) -> KeyOutcome {
        let max = ctx.cached_traces_readonly().len();
        match k.code {
            KeyCode::Up => {
                self.state.traces_cursor = self.state.traces_cursor.saturating_sub(1);
                KeyOutcome::consumed()
            }
            KeyCode::Down => {
                if self.state.traces_cursor + 1 < max {
                    self.state.traces_cursor += 1;
                }
                KeyOutcome::consumed()
            }
            KeyCode::Home => {
                self.state.traces_cursor = 0;
                KeyOutcome::consumed()
            }
            KeyCode::End => {
                self.state.traces_cursor = max.saturating_sub(1);
                KeyOutcome::consumed()
            }
            KeyCode::Enter => {
                // Reserved for Phase 6 trace-pick; consume to prevent
                // fall-through to global `q`-like keys.
                KeyOutcome::consumed()
            }
            _ => KeyOutcome::pass(),
        }
    }

    fn handle_tree_key(
        &mut self,
        ctx: &mut Ctx<'_>,
        col_idx: usize,
        _col_id: &str,
        config: &ColumnConfig,
        k: KeyEvent,
    ) -> KeyOutcome {
        let tree = self.session_tree(ctx, config);
        let flat = tree.flatten_visible(&self.state.user_collapsed);
        let max = flat.len();
        match k.code {
            KeyCode::Up => {
                self.state.cursor = self.state.cursor.saturating_sub(1);
                let mut effects = vec![self.hover_effect(ctx, config)];
                if let Some(id) = flat.get(self.state.cursor).cloned() {
                    effects.extend(self.pick_user(ctx, col_idx, config, &id));
                }
                KeyOutcome::consumed().with_effects(effects)
            }
            KeyCode::Down => {
                if self.state.cursor + 1 < max {
                    self.state.cursor += 1;
                }
                let mut effects = vec![self.hover_effect(ctx, config)];
                if let Some(id) = flat.get(self.state.cursor).cloned() {
                    effects.extend(self.pick_user(ctx, col_idx, config, &id));
                }
                KeyOutcome::consumed().with_effects(effects)
            }
            KeyCode::Home => {
                self.state.cursor = 0;
                let mut effects = vec![self.hover_effect(ctx, config)];
                if let Some(id) = flat.first().cloned() {
                    effects.extend(self.pick_user(ctx, col_idx, config, &id));
                }
                KeyOutcome::consumed().with_effects(effects)
            }
            KeyCode::End => {
                self.state.cursor = max.saturating_sub(1);
                let mut effects = vec![self.hover_effect(ctx, config)];
                if let Some(id) = flat.last().cloned() {
                    effects.extend(self.pick_user(ctx, col_idx, config, &id));
                }
                KeyOutcome::consumed().with_effects(effects)
            }
            KeyCode::Left => {
                if let Some(id) = flat.get(self.state.cursor) {
                    self.state.user_collapsed.insert(id.clone());
                }
                KeyOutcome::consumed()
            }
            KeyCode::Right => {
                if let Some(id) = flat.get(self.state.cursor) {
                    self.state.user_collapsed.remove(id);
                }
                KeyOutcome::consumed()
            }
            KeyCode::Char(' ') => {
                if let Some(id) = flat.get(self.state.cursor) {
                    if self.state.user_collapsed.contains(id) {
                        self.state.user_collapsed.remove(id);
                    } else {
                        self.state.user_collapsed.insert(id.clone());
                    }
                }
                KeyOutcome::consumed()
            }
            KeyCode::Char('+') => {
                self.state.user_collapsed.clear();
                KeyOutcome::consumed()
            }
            KeyCode::Char('-') => {
                let mut all: Vec<String> = Vec::new();
                fn walk(n: &SpanNode, out: &mut Vec<String>) {
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
                    self.state.user_collapsed.insert(id);
                }
                KeyOutcome::consumed()
            }
            KeyCode::Char('f') => {
                self.state.follow_mode = !self.state.follow_mode;
                KeyOutcome::consumed()
            }
            KeyCode::Enter
                if !k.modifiers.contains(KeyModifiers::CONTROL)
                    && !k.modifiers.contains(KeyModifiers::ALT) =>
            {
                let forward = !k.modifiers.contains(KeyModifiers::SHIFT);
                let effects = self.jump_to_match(ctx, col_idx, _col_id, config, forward);
                KeyOutcome::consumed().with_effects(effects)
            }
            _ => KeyOutcome::pass(),
        }
    }

    fn jump_to_match(
        &mut self,
        ctx: &mut Ctx<'_>,
        col_idx: usize,
        _col_id: &str,
        config: &ColumnConfig,
        forward: bool,
    ) -> Vec<ScenarioEffect> {
        let Some(session) = Self::config_session(config) else {
            return Vec::new();
        };
        let q = self.state.search.text().to_string();
        if q.is_empty() {
            return Vec::new();
        }
        let Some(resp) = ctx.cached_search_hits(&session, &q) else {
            return Vec::new();
        };
        let hits: HashSet<String> = resp.results.into_iter().map(|r| r.span_id).collect();
        if hits.is_empty() {
            return Vec::new();
        }
        let tree = self.session_tree(ctx, config);
        let flat = tree.flatten_visible(&self.state.user_collapsed);
        if flat.is_empty() {
            return Vec::new();
        }
        let n = flat.len();
        let start = self.state.cursor.min(n.saturating_sub(1));
        let new_idx = (1..=n).find_map(|step| {
            let idx = if forward {
                (start + step) % n
            } else {
                (start + n - step) % n
            };
            hits.contains(&flat[idx]).then_some(idx)
        });
        let Some(new_idx) = new_idx else {
            return Vec::new();
        };
        self.state.cursor = new_idx;
        let id = flat[new_idx].clone();
        let mut effects = vec![self.hover_effect(ctx, config)];
        effects.extend(self.pick_user(ctx, col_idx, config, &id));
        effects
    }

    // ---- render ----

    fn render_traces_list(
        &self,
        ctx: &Ctx<'_>,
        config: &ColumnConfig,
        area: Rect,
        buf: &mut Buffer,
        outcome: &mut DrawOutcome,
    ) {
        let traces = ctx.cached_traces();
        if traces.is_empty() {
            let dots = crate::tui::widgets::rolling_dots::frame_at(now_ms());
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
        let kind_filter: Option<String> = config
            .get("kind_filter")
            .and_then(|v| v.as_str())
            .map(str::to_string);
        let visible_rows = area.height as usize;
        let cursor = self
            .state
            .traces_cursor
            .min(traces.len().saturating_sub(1));
        let start = if cursor >= visible_rows {
            cursor + 1 - visible_rows
        } else {
            0
        };
        for (i_visible, idx) in (start..traces.len().min(start + visible_rows)).enumerate() {
            let t = &traces[idx];
            let id8: String = t.trace_id.chars().take(8).collect();
            let when =
                crate::tui::format::fmt_relative(t.last_seen_ns.map(|n| n as i128), None);
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

    fn render_span_detail_pane(
        &self,
        ctx: &mut Ctx<'_>,
        area: Rect,
        buf: &mut Buffer,
        tree: &[SpanNode],
        flat: &[String],
        cursor: usize,
    ) {
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
        let detail = ctx.cached_span_detail(&node.trace_id, &node.span_id);
        let mut lines: Vec<Line<'static>> = Vec::new();
        let id8: String = node.span_id.chars().take(8).collect();
        let head = format!("{}  [{}]  {}", node.name, kind_label(node.kind_class), id8);
        lines.push(Line::from(Span::styled(
            head,
            Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
        )));
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
            format!(
                "↑ parent: {}",
                parent_id.chars().take(8).collect::<String>()
            )
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
            let take_n = (inner.height as usize)
                .saturating_sub(3)
                .min(child_refs.len())
                .max(0);
            for (name, kind, id) in child_refs.iter().take(take_n) {
                let id8: String = id.chars().take(8).collect();
                lines.push(Line::from(format!("  • {name} [{kind}] {id8}")));
            }
        }
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

    fn compute_row_chips(
        &self,
        ctx: &mut Ctx<'_>,
        node: &SpanNode,
    ) -> (Vec<(String, Color)>, Option<String>) {
        let mut out: Vec<(String, Color)> = Vec::new();
        if matches!(node.kind_class, KindClass::Chat) {
            let Some(detail) = ctx.cached_span_detail(&node.trace_id, &node.span_id) else {
                return (out, None);
            };
            let Some(attrs_v) = &detail.span.attributes else {
                return (out, None);
            };
            return (out, chips::chat_text_preview(attrs_v));
        }
        if !node.is_tool_row() {
            return (out, None);
        }
        let tool_name = node.projected_tool_name().unwrap_or_default().to_string();
        if !tool_name.is_empty() {
            out.push((tool_name.clone(), crate::tui::format::hash_color(&tool_name)));
        }
        let Some(detail) = ctx.cached_span_detail(&node.trace_id, &node.span_id) else {
            return (out, None);
        };
        let Some(attrs_v) = &detail.span.attributes else {
            return (out, None);
        };
        let Some(args) = attrs::parse_tool_call_arguments(attrs_v) else {
            return (out, None);
        };
        if tool_name == "skill" {
            if let Some(s) = chips::skill_chip(&args) {
                out.push((s, Color::Green));
            }
        }
        let kind_opt = crate::tui::vendor::copilot::tool_name_mapping(&tool_name);
        for target in chips::target_chips(kind_opt, &args) {
            out.push((target, Color::Cyan));
        }
        if let Some(kind) = kind_opt {
            let (added, removed) = chips::diff_stat(kind, &args);
            if removed > 0 {
                out.push((format!("-{removed}"), Color::Red));
            }
            if added > 0 {
                out.push((format!("+{added}"), Color::Green));
            }
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
        let description = chips::tool_description_label(&args);
        (out, description)
    }

    fn compute_report_intent_titles(
        &self,
        ctx: &mut Ctx<'_>,
        tree: &[SpanNode],
    ) -> HashMap<String, String> {
        let mut out: HashMap<String, String> = HashMap::new();
        fn walk(
            node: &SpanNode,
            scn: &SpansScenario,
            ctx: &mut Ctx<'_>,
            out: &mut HashMap<String, String>,
        ) {
            let mut best: Option<&SpanNode> = None;
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
                if let Some(detail) = ctx.cached_span_detail(&child.trace_id, &child.span_id) {
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
                walk(c, scn, ctx, out);
            }
        }
        for r in tree {
            walk(r, self, ctx, &mut out);
        }
        out
    }

    fn render_popover(
        &self,
        ctx: &Ctx<'_>,
        area: Rect,
        buf: &mut Buffer,
    ) {
        let Some(which) = self.state.popover else {
            return;
        };
        match which {
            SpansPopover::Session => {
                let sessions = ctx.cached_sessions();
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
                    cursor: self.state.session_picker.cursor,
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
                    cursor: self.state.kind_picker.cursor,
                }
                .render(area, buf);
            }
        }
    }

    fn keymap_entries_default() -> Vec<(String, String)> {
        let e = |k: &str, d: &str| (k.to_string(), d.to_string());
        vec![
            e("↑ / ↓", "move row cursor (auto-selects)"),
            e("← / →", "collapse / expand focused row"),
            e("Home / End", "jump to top / bottom (auto-selects)"),
            e("+ / -", "expand all / collapse all"),
            e("Space", "toggle focused row"),
            e("f", "toggle follow mode"),
            e("/", "focus search input"),
            e("s", "open session selector"),
            e("k", "open kind filter"),
            e("Enter / Shift+Enter", "next / previous search match"),
        ]
    }

    fn keymap_entries_text_input() -> Vec<(String, String)> {
        let e = |k: &str, d: &str| (k.to_string(), d.to_string());
        vec![
            e("printable", "append character"),
            e("← / →", "move cursor"),
            e("Home / End", "jump cursor"),
            e("Backspace", "delete character left"),
            e("Enter / Shift+Enter", "next / previous match"),
            e("Esc", "exit search input (keep query)"),
            e("Delete", "clear query and exit search"),
        ]
    }
}

impl Scenario for SpansScenario {
    fn as_any(&self) -> &dyn std::any::Any { self }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any { self }

    fn draw(
        &mut self,
        ctx: &mut Ctx<'_>,
        _col_idx: usize,
        _col_id: &str,
        config: &ColumnConfig,
        area: Rect,
        buf: &mut Buffer,
        _focused: bool,
        outcome: &mut DrawOutcome,
    ) {
        if area.height < 3 {
            return;
        }

        let session = Self::config_session(config);
        let kind_filter: Option<String> = config
            .get("kind_filter")
            .and_then(|v| v.as_str())
            .map(str::to_string);

        let header_h: u16 = 2;
        let h_top = Rect::new(area.x, area.y, area.width, 1);
        let h_bot = Rect::new(area.x, area.y + 1, area.width, 1);
        let body_total = Rect::new(
            area.x,
            area.y + header_h,
            area.width,
            area.height - header_h,
        );

        let sess_label = match &session {
            Some(s) => format!("session: {}", s.chars().take(8).collect::<String>()),
            None => "session: (none) — press 's'".to_string(),
        };
        Paragraph::new(Span::styled(
            sess_label,
            Style::default().fg(Color::Cyan),
        ))
        .render(h_top, buf);

        let follow = if self.state.follow_mode {
            "[x] follow"
        } else {
            "[ ] follow"
        };
        let search_text = self.state.search.text().to_string();
        let search_label = if self.state.search_active {
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

        // No-session: traces list.
        let Some(session) = session else {
            self.render_traces_list(ctx, config, body_total, buf, outcome);
            self.render_popover(ctx, body_total, buf);
            return;
        };

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

        let tree = ctx.cached_session_span_tree(&session);
        if tree.is_empty() {
            let dots = crate::tui::widgets::rolling_dots::frame_at(now_ms());
            outcome.spinner_visible = true;
            let line = Line::from(vec![
                Span::styled(
                    "loading spans".to_string(),
                    Style::default().fg(Color::DarkGray),
                ),
                Span::styled(dots.to_string(), Style::default().fg(Color::Yellow)),
            ]);
            Paragraph::new(line).render(tree_area, buf);
            self.render_popover(ctx, body_total, buf);
            return;
        }
        let flat = tree.flatten_visible(&self.state.user_collapsed);
        let visible_rows = tree_area.height as usize;
        let cursor = self.state.cursor;
        let start = if cursor >= visible_rows {
            cursor + 1 - visible_rows
        } else {
            0
        };
        let hit_set: Option<HashSet<String>> = if !search_text.is_empty() {
            ctx.cached_search_hits(&session, &search_text)
                .map(|resp| resp.results.into_iter().map(|r| r.span_id).collect())
        } else {
            None
        };
        let report_titles = self.compute_report_intent_titles(ctx, &tree);
        let kf_lower = kind_filter.as_deref().map(str::to_lowercase);
        let now = now_ms();

        for (i_visible, flat_idx) in
            (start..flat.len().min(start + visible_rows)).enumerate()
        {
            let row_id = &flat[flat_idx];
            let Some((node, depth)) = tree.find_with_depth(row_id) else {
                continue;
            };
            if node.ingestion_state == "placeholder" {
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
            let (chips, description) = self.compute_row_chips(ctx, node);
            let report_title = report_titles.get(&node.span_id).cloned();
            let row_area = Rect::new(tree_area.x, row_y, tree_area.width, 1);
            SpansTreeRow {
                node,
                depth,
                focused,
                collapsed: self.state.user_collapsed.contains(row_id),
                row_bg,
                row_dim,
                chips: &chips,
                description: description.as_deref(),
                report_title: report_title.as_deref(),
                now_ms: now,
            }
            .render(row_area, buf);
        }

        if detail_h >= 3 {
            self.render_span_detail_pane(ctx, detail_area, buf, &tree, &flat, cursor);
        }
        self.render_popover(ctx, body_total, buf);
    }

    fn handle_key(
        &mut self,
        ctx: &mut Ctx<'_>,
        col_idx: usize,
        col_id: &str,
        config: &ColumnConfig,
        k: KeyEvent,
    ) -> KeyOutcome {
        // Mode dispatch in precedence order: text-input first, then
        // popover, then main keys. The App's Layer 1 / Layer 2
        // already gated us so for example text_input_active and
        // popover_active are pre-checked, but defending here keeps
        // behaviour correct under direct test invocation too.
        if self.state.search_active {
            return self.handle_text_input_key(ctx, col_idx, col_id, config, k);
        }
        if let Some(popover) = self.state.popover {
            return self.handle_popover_key(ctx, col_idx, popover, k);
        }
        self.handle_main_key(ctx, col_idx, col_id, config, k)
    }

    fn keymap_entries(&self, _config: &ColumnConfig) -> Vec<(String, String)> {
        if self.state.search_active {
            Self::keymap_entries_text_input()
        } else {
            Self::keymap_entries_default()
        }
    }

    fn on_ws_batch(
        &mut self,
        ctx: &mut Ctx<'_>,
        col_idx: usize,
        _col_id: &str,
        config: &ColumnConfig,
        meta: &WsBatchMeta,
    ) -> Vec<ScenarioEffect> {
        if !meta.touches_spans {
            return Vec::new();
        }
        self.advance_follow_mode_if_engaged(ctx, col_idx, config)
    }

    fn on_cache_changed(
        &mut self,
        ctx: &mut Ctx<'_>,
        col_idx: usize,
        _col_id: &str,
        config: &ColumnConfig,
    ) -> Vec<ScenarioEffect> {
        self.advance_follow_mode_if_engaged(ctx, col_idx, config)
    }

    fn tick(
        &mut self,
        _ctx: &mut Ctx<'_>,
        _col_idx: usize,
        _col_id: &str,
        _config: &ColumnConfig,
        now_ms: u64,
    ) -> Vec<ScenarioEffect> {
        let _ = self.state.reveal.drain_due(now_ms);
        Vec::new()
    }

    fn next_anim_deadline(&self, _config: &ColumnConfig) -> Option<u64> {
        self.state.reveal.queue.first().map(|&(_, at)| at)
    }

    fn text_input_active(&self, _config: &ColumnConfig) -> bool {
        self.state.search_active
    }

    fn popover_active(&self, _config: &ColumnConfig) -> bool {
        self.state.popover.is_some()
    }
}

// (no helpers — col_idx is unused in `draw`'s renderer body; the parameter
// is prefixed with `_` to silence the lint.)
