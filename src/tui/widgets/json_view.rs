//! `JsonView` — pretty-printed JSON with an optional collapsed summary. The
//! terminal analog of the web `JsonView` component.
//!
//! When `collapsed` is true the widget renders a single-line `"json…"` summary
//! (the closed `<details>` analog); when open it renders
//! `serde_json::to_string_pretty(value)` (2-space indent), falling back to
//! `format!("{value}")` if stringification ever fails — it never panics.
//!
//! In scenario context the open body is composed inside a
//! `SearchableTextBlock` so the per-block search affordance works; this widget
//! is the reusable standalone form and exposes [`JsonView::pretty`] for the
//! composed path.
//!
//! Source for (shared `frontend/llr/`):
//! - `JsonView pretty prints with optional collapse`
//!
//! Also source for (new `frontend/tui/llr/`):
//! - `TUI JsonView collapsed default closed`

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Widget};
use serde_json::Value;

/// The closed-summary glyph + label.
pub const SUMMARY_CLOSED: &str = "▸ json…";
/// The open-summary glyph + label.
pub const SUMMARY_OPEN: &str = "▾ json…";

/// Stateless JSON renderer.
pub struct JsonView<'a> {
    /// The value to render.
    pub value: &'a Value,
    /// When true, render only the `"json…"` summary line.
    pub collapsed: bool,
}

impl<'a> JsonView<'a> {
    /// Pretty-print `value` with a 2-space indent; never panics.
    pub fn pretty(value: &Value) -> String {
        serde_json::to_string_pretty(value).unwrap_or_else(|_| format!("{value}"))
    }

    /// Render into `area`. A collapsed view shows the summary line only; an
    /// open view shows the summary followed by the pretty JSON.
    pub fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width == 0 || area.height == 0 {
            return;
        }
        let mut lines: Vec<Line<'static>> = Vec::new();
        let summary = if self.collapsed {
            SUMMARY_CLOSED
        } else {
            SUMMARY_OPEN
        };
        lines.push(Line::from(Span::styled(
            summary.to_string(),
            Style::default().fg(Color::DarkGray).add_modifier(Modifier::DIM),
        )));
        if !self.collapsed {
            for line in Self::pretty(self.value).lines() {
                lines.push(Line::from(Span::styled(
                    line.to_string(),
                    Style::default().fg(Color::Gray),
                )));
            }
        }
        Paragraph::new(lines).render(area, buf);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::buffer::Buffer;
    use ratatui::layout::Rect;
    use serde_json::json;

    fn buf_text(buf: &Buffer) -> String {
        let mut s = String::new();
        for y in 0..buf.area.height {
            for x in 0..buf.area.width {
                s.push_str(buf[(x, y)].symbol());
            }
            s.push('\n');
        }
        s
    }

    #[test]
    fn pretty_uses_two_space_indent() {
        let v = json!({"a": 1});
        let p = JsonView::pretty(&v);
        assert!(p.contains("  \"a\": 1"), "got: {p:?}");
    }

    #[test]
    fn pretty_never_panics_on_any_value() {
        // serde_json::Value always stringifies; exercise the fallback shape.
        let v = json!(null);
        assert_eq!(JsonView::pretty(&v), "null");
    }

    #[test]
    fn collapsed_renders_summary_only() {
        let area = Rect::new(0, 0, 20, 4);
        let mut buf = Buffer::empty(area);
        let v = json!({"a": 1});
        JsonView { value: &v, collapsed: true }.render(area, &mut buf);
        let t = buf_text(&buf);
        assert!(t.contains("json…"), "got: {t:?}");
        assert!(!t.contains("\"a\""), "collapsed must hide body; got: {t:?}");
    }

    #[test]
    fn open_renders_body() {
        let area = Rect::new(0, 0, 20, 6);
        let mut buf = Buffer::empty(area);
        let v = json!({"a": 1});
        JsonView { value: &v, collapsed: false }.render(area, &mut buf);
        let t = buf_text(&buf);
        assert!(t.contains("\"a\""), "got: {t:?}");
    }
}
