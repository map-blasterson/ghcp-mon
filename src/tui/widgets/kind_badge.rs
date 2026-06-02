//! Kind badge — small label with hash-coloured background. Label rename
//! table per `Kind badge label renames raw kinds`.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Span;
use ratatui::widgets::Widget;

use crate::tui::format::hash_color;
use crate::tui::model::KindClass;

/// Map a [`KindClass`] to its display label per the rename LLR.
pub fn kind_label(k: KindClass) -> &'static str {
    match k {
        KindClass::ExecuteTool => "tool",
        KindClass::ExternalTool => "external",
        KindClass::InvokeAgent => "agent",
        KindClass::Other => "pending",
        KindClass::Chat => "chat",
    }
}

pub struct KindBadge {
    pub kind: KindClass,
    /// Override label (e.g., when a tool-name should colour the badge).
    pub hash_seed: Option<String>,
}

impl KindBadge {
    pub fn new(kind: KindClass) -> Self {
        Self {
            kind,
            hash_seed: None,
        }
    }

    pub fn with_seed(mut self, seed: impl Into<String>) -> Self {
        self.hash_seed = Some(seed.into());
        self
    }
}

impl Widget for KindBadge {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width == 0 || area.height == 0 {
            return;
        }
        let label = kind_label(self.kind);
        let seed = self.hash_seed.as_deref().unwrap_or(label);
        let fg = hash_color(seed);
        let style = Style::default().fg(fg).add_modifier(Modifier::BOLD);
        let text = format!("[{label}]");
        let span = Span::styled(text, style);
        buf.set_span(area.x, area.y, &span, area.width);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn label_renames() {
        assert_eq!(kind_label(KindClass::ExecuteTool), "tool");
        assert_eq!(kind_label(KindClass::ExternalTool), "external");
        assert_eq!(kind_label(KindClass::InvokeAgent), "agent");
        assert_eq!(kind_label(KindClass::Other), "pending");
        assert_eq!(kind_label(KindClass::Chat), "chat");
    }
}
