//! Minimal `[x]` / `[ ]` toggle for the follow-mode checkbox.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::Span;
use ratatui::widgets::Widget;

pub struct Checkbox<'a> {
    pub checked: bool,
    pub label: &'a str,
    pub focused: bool,
}

impl<'a> Widget for Checkbox<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width < 4 || area.height == 0 {
            return;
        }
        let glyph = if self.checked { "[x]" } else { "[ ]" };
        let color = if self.focused {
            Color::Yellow
        } else {
            Color::White
        };
        let s = format!("{glyph} {}", self.label);
        let span = Span::styled(s, Style::default().fg(color));
        buf.set_span(area.x, area.y, &span, area.width);
    }
}
