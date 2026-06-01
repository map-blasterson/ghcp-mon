//! Single-cell status dot widget. Green when WS is connected, amber while
//! reconnecting (< 5 failures), red on `Error`. Renders inline (the title
//! the LLR mandates lives in the surrounding parent's `Block::title`).

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::Span;
use ratatui::widgets::Widget;

use crate::tui::ws::WsStatus;

pub struct StatusDot {
    pub status: WsStatus,
}

impl StatusDot {
    pub fn new(status: WsStatus) -> Self {
        Self { status }
    }

    /// Title text per the LLR ("connected" when on, "disconnected" when off,
    /// plus reconnect/error variants for parity with our extended state set).
    pub fn title(&self) -> &'static str {
        match self.status {
            WsStatus::Connected => "connected",
            WsStatus::Connecting => "connecting",
            WsStatus::Reconnecting => "reconnecting",
            WsStatus::Error => "ws error",
        }
    }

    fn color(&self) -> Color {
        match self.status {
            WsStatus::Connected => Color::Green,
            WsStatus::Connecting | WsStatus::Reconnecting => Color::Yellow,
            WsStatus::Error => Color::Red,
        }
    }
}

impl Widget for StatusDot {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width == 0 || area.height == 0 {
            return;
        }
        let span = Span::styled("●", Style::default().fg(self.color()));
        buf.set_span(area.x, area.y, &span, area.width);
    }
}
