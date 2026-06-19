//! Modal overlay that displays the active keyboard bindings for the current
//! focus context.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Widget, Wrap};

/// Modal overlay renderer.
pub struct KeymapOverlay {
    pub entries: Vec<(String, String)>,
}

impl Widget for KeymapOverlay {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width < 20 || area.height < 7 {
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
            .title("keymap (? to close)")
            .style(Style::default().bg(Color::Black).fg(Color::White));

        let key_width = self
            .entries
            .iter()
            .map(|(key, _)| key.chars().count())
            .max()
            .unwrap_or(3);
        let body_lines: Vec<Line<'static>> = self
            .entries
            .into_iter()
            .map(|(key, desc)| {
                Line::from(vec![
                    Span::styled(
                        format!("{key:<key_width$}"),
                        Style::default()
                            .fg(Color::Yellow)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::raw("  "),
                    Span::raw(desc),
                ])
            })
            .collect();

        Paragraph::new(body_lines)
            .block(block)
            .wrap(Wrap { trim: false })
            .render(modal, buf);
    }
}
