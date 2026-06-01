//! Popover dropdown for the session selector and kind filter. State is
//! minimal: open/closed + cursor index. The renderer overlays a centered
//! list when open; keys `↑`/`↓` move cursor, `Enter` selects, `Esc` closes.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Span;
use ratatui::widgets::{Block, Borders, Clear, Widget};

#[derive(Debug, Default, Clone)]
pub struct SelectState {
    pub open: bool,
    pub cursor: usize,
}

impl SelectState {
    pub fn open(&mut self, initial: usize) {
        self.open = true;
        self.cursor = initial;
    }
    pub fn close(&mut self) {
        self.open = false;
    }
    pub fn move_cursor(&mut self, dir: i32, max: usize) {
        if max == 0 {
            self.cursor = 0;
            return;
        }
        let n = max as i32;
        let cur = self.cursor as i32;
        self.cursor = ((cur + dir).rem_euclid(n)) as usize;
    }
}

pub struct SelectPopover<'a> {
    pub title: &'a str,
    pub options: &'a [String],
    pub cursor: usize,
}

impl<'a> Widget for SelectPopover<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width < 20 || area.height < 5 {
            return;
        }
        let w = area.width.saturating_sub(8).min(60);
        let visible = self.options.len().min(area.height.saturating_sub(4) as usize);
        let h = (visible as u16).max(3) + 2;
        let x = area.x + (area.width.saturating_sub(w)) / 2;
        let y = area.y + (area.height.saturating_sub(h)) / 2;
        let modal = Rect::new(x, y, w, h);
        Clear.render(modal, buf);
        let block = Block::default()
            .borders(Borders::ALL)
            .title(format!(" {} ", self.title))
            .style(Style::default().bg(Color::Black).fg(Color::White));
        let inner = block.inner(modal);
        block.render(modal, buf);
        if self.options.is_empty() {
            let span = Span::styled(
                "(no options)".to_string(),
                Style::default().fg(Color::DarkGray),
            );
            buf.set_span(inner.x, inner.y, &span, inner.width);
            return;
        }
        // Scroll the list so cursor is visible.
        let visible_rows = inner.height as usize;
        let start = self.cursor.saturating_sub(visible_rows.saturating_sub(1));
        for (i, opt) in self
            .options
            .iter()
            .enumerate()
            .skip(start)
            .take(visible_rows)
        {
            let y = inner.y + (i - start) as u16;
            let focused = i == self.cursor;
            let style = if focused {
                Style::default()
                    .bg(Color::Yellow)
                    .fg(Color::Black)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };
            let span = Span::styled(opt.clone(), style);
            buf.set_span(inner.x, y, &span, inner.width);
        }
    }
}
