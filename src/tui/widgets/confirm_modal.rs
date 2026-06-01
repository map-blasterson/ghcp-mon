//! Centered modal with a title, prompt, and `[ y ]es` / `[ N ]o` choices.
//! Default selection is `No` per `TUI Confirm modal default no`.

use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Widget, Wrap};

#[derive(Debug, Clone)]
pub struct ConfirmModalState {
    pub open: bool,
    pub title: String,
    pub prompt: String,
    /// `false` = No (default), `true` = Yes.
    pub yes_selected: bool,
}

impl ConfirmModalState {
    pub fn new() -> Self {
        Self {
            open: false,
            title: String::new(),
            prompt: String::new(),
            yes_selected: false,
        }
    }

    pub fn open(&mut self, title: impl Into<String>, prompt: impl Into<String>) {
        self.open = true;
        self.title = title.into();
        self.prompt = prompt.into();
        self.yes_selected = false; // default = No
    }

    /// Returns `Some(confirmed)` if the modal was dismissed, where
    /// `confirmed = true` means the user picked Yes. Returns `None` if the
    /// key did not dismiss the modal.
    pub fn handle_key(&mut self, k: KeyEvent) -> Option<bool> {
        if !self.open {
            return None;
        }
        match k.code {
            KeyCode::Char('y') | KeyCode::Char('Y') => {
                self.open = false;
                Some(true)
            }
            KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                self.open = false;
                Some(false)
            }
            KeyCode::Left | KeyCode::Right | KeyCode::Tab => {
                self.yes_selected = !self.yes_selected;
                None
            }
            KeyCode::Enter => {
                let r = self.yes_selected;
                self.open = false;
                Some(r)
            }
            _ => None,
        }
    }
}

impl Default for ConfirmModalState {
    fn default() -> Self {
        Self::new()
    }
}

pub struct ConfirmModalView<'a> {
    pub state: &'a ConfirmModalState,
}

impl<'a> Widget for ConfirmModalView<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if !self.state.open || area.width < 20 || area.height < 7 {
            return;
        }
        let w = area.width.min(70).max(40);
        let h = 7u16;
        let x = area.x + (area.width.saturating_sub(w)) / 2;
        let y = area.y + (area.height.saturating_sub(h)) / 2;
        let modal = Rect::new(x, y, w, h);
        Clear.render(modal, buf);
        let block = Block::default()
            .borders(Borders::ALL)
            .title(format!(" {} ", self.state.title))
            .style(Style::default().bg(Color::Black).fg(Color::White));
        let inner = block.inner(modal);
        block.render(modal, buf);

        // Prompt
        let prompt = Paragraph::new(self.state.prompt.clone())
            .wrap(Wrap { trim: false })
            .style(Style::default().fg(Color::White));
        let prompt_area = Rect::new(inner.x, inner.y, inner.width, inner.height.saturating_sub(2));
        prompt.render(prompt_area, buf);

        // Buttons row
        let y_btn = inner.y + inner.height.saturating_sub(1);
        let (yes_style, no_style) = if self.state.yes_selected {
            (
                Style::default()
                    .bg(Color::Yellow)
                    .fg(Color::Black)
                    .add_modifier(Modifier::BOLD),
                Style::default().fg(Color::White),
            )
        } else {
            (
                Style::default().fg(Color::White),
                Style::default()
                    .bg(Color::Yellow)
                    .fg(Color::Black)
                    .add_modifier(Modifier::BOLD),
            )
        };
        let line = Line::from(vec![
            Span::styled("[ y ]es", yes_style),
            Span::raw("    "),
            Span::styled("[ N ]o", no_style),
        ]);
        let p = Paragraph::new(line).alignment(ratatui::layout::Alignment::Center);
        p.render(Rect::new(inner.x, y_btn, inner.width, 1), buf);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::crossterm::event::{KeyEventKind, KeyEventState, KeyModifiers};

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent {
            code,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        }
    }

    #[test]
    fn default_selection_is_no() {
        let mut m = ConfirmModalState::new();
        m.open("title", "prompt");
        assert!(!m.yes_selected);
        // Pressing Enter without moving → confirms with No.
        let r = m.handle_key(key(KeyCode::Enter)).unwrap();
        assert!(!r);
    }

    #[test]
    fn esc_dismisses_as_no() {
        let mut m = ConfirmModalState::new();
        m.open("title", "prompt");
        let r = m.handle_key(key(KeyCode::Esc)).unwrap();
        assert!(!r);
        assert!(!m.open);
    }

    #[test]
    fn y_confirms_n_cancels() {
        let mut m = ConfirmModalState::new();
        m.open("t", "p");
        assert_eq!(m.handle_key(key(KeyCode::Char('y'))), Some(true));
        m.open("t", "p");
        assert_eq!(m.handle_key(key(KeyCode::Char('n'))), Some(false));
    }

    #[test]
    fn arrow_toggles_selection_then_enter_confirms() {
        let mut m = ConfirmModalState::new();
        m.open("t", "p");
        m.handle_key(key(KeyCode::Right));
        assert!(m.yes_selected);
        let r = m.handle_key(key(KeyCode::Enter)).unwrap();
        assert!(r);
    }
}
