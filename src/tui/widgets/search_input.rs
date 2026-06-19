//! Single-line text input widget for the Spans header searchbox. Supports
//! `←` `→` cursor moves, `Home` / `End`, `Backspace`, `Delete`, and
//! character entry. Used in *text-input* mode (per Key-Dispatch Policy);
//! the caller drains [`SearchInput::on_change`] to implement debounce.

use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Span;
use ratatui::widgets::Widget;

#[derive(Debug, Default, Clone)]
pub struct SearchInput {
    text: String,
    cursor: usize, // byte offset
    /// Tracks whether the last `handle_key` mutated the buffer; caller polls
    /// via [`take_changed`].
    changed: bool,
}

impl SearchInput {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn set_text(&mut self, s: impl Into<String>) {
        self.text = s.into();
        self.cursor = self.text.len();
        self.changed = true;
    }

    pub fn clear(&mut self) {
        self.text.clear();
        self.cursor = 0;
        self.changed = true;
    }

    pub fn take_changed(&mut self) -> bool {
        std::mem::take(&mut self.changed)
    }

    /// Handle one key event. Returns `true` if the key was consumed.
    /// `Esc` is **not** consumed here — the caller decides whether to exit
    /// input mode.
    pub fn handle_key(&mut self, k: KeyEvent) -> bool {
        match k.code {
            KeyCode::Char(c)
                if !k.modifiers.contains(KeyModifiers::CONTROL)
                    && !k.modifiers.contains(KeyModifiers::ALT) =>
            {
                self.text.insert(self.cursor, c);
                self.cursor += c.len_utf8();
                self.changed = true;
                true
            }
            KeyCode::Backspace => {
                if self.cursor == 0 {
                    return true;
                }
                let prev = prev_char_boundary(&self.text, self.cursor);
                self.text.replace_range(prev..self.cursor, "");
                self.cursor = prev;
                self.changed = true;
                true
            }
            KeyCode::Delete => {
                if self.cursor >= self.text.len() {
                    return true;
                }
                let next = next_char_boundary(&self.text, self.cursor);
                self.text.replace_range(self.cursor..next, "");
                self.changed = true;
                true
            }
            KeyCode::Left => {
                if self.cursor > 0 {
                    self.cursor = prev_char_boundary(&self.text, self.cursor);
                }
                true
            }
            KeyCode::Right => {
                if self.cursor < self.text.len() {
                    self.cursor = next_char_boundary(&self.text, self.cursor);
                }
                true
            }
            KeyCode::Home => {
                self.cursor = 0;
                true
            }
            KeyCode::End => {
                self.cursor = self.text.len();
                true
            }
            _ => false,
        }
    }
}

fn prev_char_boundary(s: &str, i: usize) -> usize {
    let mut j = i.saturating_sub(1);
    while !s.is_char_boundary(j) && j > 0 {
        j -= 1;
    }
    j
}

fn next_char_boundary(s: &str, i: usize) -> usize {
    let mut j = i + 1;
    while j < s.len() && !s.is_char_boundary(j) {
        j += 1;
    }
    j.min(s.len())
}

/// Lightweight render — single line with a `> ` prompt; cursor is shown as
/// reverse-video.
pub struct SearchInputView<'a> {
    pub state: &'a SearchInput,
    pub focused: bool,
    pub prompt: &'a str,
}

impl<'a> Widget for SearchInputView<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width == 0 || area.height == 0 {
            return;
        }
        let style = if self.focused {
            Style::default().fg(Color::White)
        } else {
            Style::default().fg(Color::DarkGray)
        };
        let prompt = Span::styled(self.prompt.to_string(), style);
        buf.set_span(area.x, area.y, &prompt, area.width);
        let xoff = self.prompt.chars().count() as u16;
        if xoff >= area.width {
            return;
        }
        let body = Span::styled(self.state.text.to_string(), style);
        buf.set_span(area.x + xoff, area.y, &body, area.width - xoff);
        if self.focused {
            // Render cursor as a reverse-video block at cursor position.
            let cx = area.x
                + xoff
                + self.state.text[..self.state.cursor].chars().count() as u16;
            if cx < area.x + area.width {
                let ch: String = self.state.text[self.state.cursor..]
                    .chars()
                    .next()
                    .map(|c| c.to_string())
                    .unwrap_or_else(|| " ".to_string());
                let cs = Span::styled(
                    ch,
                    Style::default()
                        .bg(Color::White)
                        .fg(Color::Black)
                        .add_modifier(Modifier::REVERSED),
                );
                buf.set_span(cx, area.y, &cs, 1);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::crossterm::event::{KeyEvent, KeyEventKind, KeyEventState};

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent {
            code,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        }
    }

    #[test]
    fn typing_and_backspace() {
        let mut s = SearchInput::new();
        for c in "hi".chars() {
            assert!(s.handle_key(key(KeyCode::Char(c))));
        }
        assert_eq!(s.text(), "hi");
        s.handle_key(key(KeyCode::Backspace));
        assert_eq!(s.text(), "h");
    }

    #[test]
    fn cursor_movement_and_delete() {
        let mut s = SearchInput::new();
        s.set_text("abcd");
        s.handle_key(key(KeyCode::Home));
        s.handle_key(key(KeyCode::Right));
        s.handle_key(key(KeyCode::Delete));
        assert_eq!(s.text(), "acd");
    }
}
