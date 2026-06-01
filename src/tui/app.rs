//! Top-level TUI App: state, event-loop, draw. The loop obeys the
//! drain-then-draw rule: every pending [`AppEvent`] is drained via
//! `try_recv` before `terminal.draw` runs once.

use std::sync::Arc;

use anyhow::Result;
use ratatui::DefaultTerminal;
use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::crossterm::event::{
    DisableMouseCapture, EnableMouseCapture, KeyCode, KeyModifiers,
};
use ratatui::crossterm::execute;
use tokio::sync::mpsc;
use tracing::{debug, info};

use crate::tui::api::ApiClient;
use crate::tui::cache::QueryCache;
use crate::tui::event::{
    AppEvent, spawn_crossterm_reader, spawn_tick, spawn_ws_coalescer,
};
use crate::tui::live_feed::LiveFeed;
use crate::tui::persist;
use crate::tui::scenarios::render_placeholder;
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
    #[allow(dead_code)]
    pub cache: Arc<QueryCache>,
    #[allow(dead_code)]
    pub live_feed: Arc<LiveFeed>,
    #[allow(dead_code)]
    pub api: ApiClient,
    pub ws: WsBus,
    pub log_buffer: LogBuffer,
    pub focused_column: Option<usize>,
    pub log_overlay_visible: bool,
    pub mouse_enabled: bool,
    pub add_column_cursor: usize,
    pub status: WsStatus,
    pub last_ws_event: Option<String>,
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
        }
    }

    /// Process one drained event. Returns `Ok(true)` if the loop should
    /// quit.
    pub fn handle(&mut self, ev: AppEvent) -> Result<bool> {
        match ev {
            AppEvent::Quit => return Ok(true),
            AppEvent::Tick => {}
            AppEvent::Crossterm(crossterm::event::Event::Key(k))
                if k.kind == crossterm::event::KeyEventKind::Press =>
            {
                if self.handle_key(k)? {
                    return Ok(true);
                }
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

    fn handle_key(&mut self, k: crossterm::event::KeyEvent) -> Result<bool> {
        // Global keys: log overlay swallows nothing else but `?` and `Esc`
        // when visible.
        if self.log_overlay_visible {
            match k.code {
                KeyCode::Char('?') | KeyCode::Esc => {
                    self.log_overlay_visible = false;
                }
                _ => {}
            }
            return Ok(false);
        }

        match (k.code, k.modifiers) {
            (KeyCode::Char('q'), m) if !m.contains(KeyModifiers::SHIFT) => {
                return Ok(true);
            }
            (KeyCode::Char('c'), m) if m.contains(KeyModifiers::CONTROL) => {
                return Ok(true);
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

    fn cycle_focus(&mut self, dir: i32) {
        let n = self.workspace.columns.len();
        if n == 0 {
            self.focused_column = None;
            return;
        }
        let cur = self.focused_column.unwrap_or(0) as i32;
        let next = ((cur + dir).rem_euclid(n as i32)) as usize;
        self.focused_column = Some(next);
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
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(1), Constraint::Min(0)])
            .split(area);
        self.draw_top_bar(frame, chunks[0]);
        self.draw_workspace(frame, chunks[1]);
        if self.log_overlay_visible {
            let lines = self.log_buffer.snapshot();
            frame.render_widget(LogOverlay { lines }, area);
        }
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
            // Phase 0 always renders the placeholder body.
            let cfg = col.config.clone();
            let st = col.scenario_type;
            let buf: &mut Buffer = frame.buffer_mut();
            render_placeholder(inner, buf, st, &cfg);
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
        App::new(api, ws, LogBuffer::new(), false)
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
}
