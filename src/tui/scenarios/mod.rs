//! Scenario dispatch: Phase 1 ships real renderers for LiveSessions and
//! Spans; later phases add ToolDetail, ChatDetail, and FileTouches. Only
//! RawBrowser keeps the Phase-0 placeholder renderer (which dumps
//! `column.config` so cross-column routing can be evaluated visually).

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Widget, Wrap};

use crate::tui::workspace::{ColumnConfig, ScenarioType};

pub mod live_sessions;
pub mod spans;
pub mod tool_detail;
pub mod chat_detail;
pub mod file_touches;

/// Commands a scenario emits back to [`crate::tui::app::App`] after handling
/// a key. Scenarios MUST NOT mutate workspace columns, the confirm modal,
/// the hovered-chat pubsub, or trigger persistence directly — they return
/// effects, and the App applies them after the per-scenario borrow ends.
///
/// This decouples scenarios from \[`App`\]'s internals and avoids the
/// repeated `&mut self.workspace.columns` + `persist::save(&self.workspace)`
/// + `confirm_modal.open(...)` pattern that previously forced every
/// scenario handler to live as a method on App. It also keeps the borrow
/// graph simple: a scenario takes `&mut state` + read-only context and
/// returns owned data.
///
/// More effect variants will be added as the remaining scenarios migrate
/// (Phase 11). The first user is `live_sessions` (Phase 9 proof).
#[derive(Debug, Clone)]
pub enum ScenarioEffect {
    /// Set the column at `origin_col_idx`'s session to `cid` and clear the
    /// session everywhere else (per
    /// `Selecting session propagates to dependent columns`).
    PropagateSession {
        origin_col_idx: usize,
        cid: String,
    },
    /// Open the confirm-delete modal for a session id; on confirmation
    /// [`App`] will call `do_delete_session`.
    ConfirmDeleteSession { cid: String },
    /// Persist the workspace to disk.
    PersistWorkspace,
}

/// Render a Phase-0 placeholder for any scenario type still without a real
/// renderer. The placeholder dumps the column's `config` so cross-column
/// routing can be inspected without rendering the real column.
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
