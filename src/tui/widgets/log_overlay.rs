//! In-process tracing log buffer + modal overlay widget. The buffer is the
//! tail of the most recent records emitted by the tracing-subscriber layer
//! installed in `main.rs`; the overlay is a centered Block that renders the
//! tail when the `?` key is pressed.

use std::collections::VecDeque;
use std::fmt::Write as _;
use std::sync::{Arc, Mutex};

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Widget, Wrap};
use tracing::field::{Field, Visit};
use tracing::{Event, Subscriber};
use tracing_subscriber::Layer;
use tracing_subscriber::layer::Context;

pub const LOG_BUFFER_CAP: usize = 500;

/// Thread-safe ring buffer of formatted log lines.
#[derive(Clone, Default)]
pub struct LogBuffer {
    inner: Arc<Mutex<VecDeque<String>>>,
}

impl LogBuffer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&self, line: String) {
        let mut g = self.inner.lock().unwrap();
        g.push_back(line);
        while g.len() > LOG_BUFFER_CAP {
            g.pop_front();
        }
    }

    pub fn snapshot(&self) -> Vec<String> {
        self.inner.lock().unwrap().iter().cloned().collect()
    }

    pub fn len(&self) -> usize {
        self.inner.lock().unwrap().len()
    }
}

/// Tracing layer that mirrors every emitted event into a [`LogBuffer`].
pub struct LogBufferLayer {
    pub buffer: LogBuffer,
}

impl LogBufferLayer {
    pub fn new(buffer: LogBuffer) -> Self {
        Self { buffer }
    }
}

struct FieldVisitor {
    out: String,
}

impl Visit for FieldVisitor {
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        let _ = write!(self.out, " {}={:?}", field.name(), value);
    }

    fn record_str(&mut self, field: &Field, value: &str) {
        let _ = write!(self.out, " {}={}", field.name(), value);
    }
}

impl<S: Subscriber + for<'a> tracing_subscriber::registry::LookupSpan<'a>> Layer<S>
    for LogBufferLayer
{
    fn on_event(&self, event: &Event<'_>, _ctx: Context<'_, S>) {
        let meta = event.metadata();
        let mut v = FieldVisitor { out: String::new() };
        event.record(&mut v);
        let line = format!(
            "{:>5} {}{}",
            meta.level().to_string(),
            meta.target(),
            v.out
        );
        self.buffer.push(line);
    }
}

/// Modal overlay renderer.
pub struct LogOverlay {
    pub lines: Vec<String>,
}

impl Widget for LogOverlay {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width < 10 || area.height < 5 {
            return;
        }
        let w = area.width.saturating_sub(4);
        let h = area.height.saturating_sub(4);
        let x = area.x + 2;
        let y = area.y + 2;
        let modal = Rect::new(x, y, w, h);
        Clear.render(modal, buf);
        let block = Block::default()
            .borders(Borders::ALL)
            .title("logs (? to close)")
            .style(Style::default().bg(Color::Black).fg(Color::White));
        // Show newest at the top.
        let visible_rows = modal.height.saturating_sub(2) as usize;
        let take_from = self.lines.len().saturating_sub(visible_rows);
        let body_lines: Vec<Line<'static>> = self.lines[take_from..]
            .iter()
            .rev()
            .map(|l| Line::from(Span::raw(l.clone())))
            .collect();
        Paragraph::new(body_lines)
            .block(block)
            .wrap(Wrap { trim: false })
            .render(modal, buf);
    }
}
