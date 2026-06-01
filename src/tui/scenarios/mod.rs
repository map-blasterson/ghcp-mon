//! Scenario placeholders. Phase 0 ships **no** real scenario rendering — but
//! the unimplemented ones each render a placeholder that shows their
//! `column.config` so Phase 1 cross-column routing can be evaluated.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Widget, Wrap};

use crate::tui::workspace::{ColumnConfig, ScenarioType};

/// Render a Phase-0 placeholder for any scenario type. Body is:
///
/// ```text
/// <scenario> — not yet implemented in this phase
///
/// config:
///   key = value
///   key = value
/// ```
pub fn render_placeholder(
    area: Rect,
    buf: &mut Buffer,
    scenario_type: ScenarioType,
    config: &ColumnConfig,
) {
    let mut lines: Vec<Line<'static>> = Vec::new();
    lines.push(Line::from(Span::styled(
        format!(
            "{} — not yet implemented in this phase",
            scenario_type.default_title()
        ),
        Style::default().add_modifier(Modifier::DIM | Modifier::ITALIC),
    )));
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "config:",
        Style::default().fg(Color::Cyan),
    )));
    if config.is_empty() {
        lines.push(Line::from(Span::raw("  (empty)")));
    } else {
        let mut keys: Vec<&String> = config.keys().collect();
        keys.sort();
        for k in keys {
            let v = &config[k];
            lines.push(Line::from(format!(
                "  {k} = {}",
                toml::to_string(v).unwrap_or_else(|_| format!("{v:?}")).trim()
            )));
        }
    }
    Paragraph::new(lines).wrap(Wrap { trim: false }).render(area, buf);
}
