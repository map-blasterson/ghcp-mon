//! Workspace model — columns, context-widget state, schema version.
//! Mirrors `web/src/state/workspace.ts`. Persisted to disk by
//! [`crate::tui::persist`].

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Persisted schema version. Bumping this triggers the migrate-drop path in
/// [`crate::tui::persist::migrate`].
pub const SCHEMA_VERSION: u32 = 6;

/// Scenario type — one per column body renderer. Order mirrors the web side's
/// `SCENARIOS` union.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScenarioType {
    LiveSessions,
    Spans,
    ToolDetail,
    ChatDetail,
    FileTouches,
    RawBrowser,
}

impl ScenarioType {
    pub fn all() -> &'static [ScenarioType] {
        &[
            ScenarioType::LiveSessions,
            ScenarioType::Spans,
            ScenarioType::ToolDetail,
            ScenarioType::ChatDetail,
            ScenarioType::FileTouches,
            ScenarioType::RawBrowser,
        ]
    }

    /// Human label, used in the column header.
    pub fn default_title(&self) -> &'static str {
        match self {
            ScenarioType::LiveSessions => "Sessions",
            ScenarioType::Spans => "Spans",
            ScenarioType::ToolDetail => "Tool detail",
            ScenarioType::ChatDetail => "Chat detail",
            ScenarioType::FileTouches => "File touches",
            ScenarioType::RawBrowser => "Raw",
        }
    }
}

/// Per-column config — open-ended TOML map matching the web's `ColumnConfig`.
pub type ColumnConfig = HashMap<String, toml::Value>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Column {
    pub id: String,
    pub scenario_type: ScenarioType,
    pub title: String,
    #[serde(default)]
    pub config: ColumnConfig,
    pub width: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Workspace {
    pub schema_version: u32,
    pub columns: Vec<Column>,
    pub context_widget_height_rows: u16,
    pub context_widget_visible: bool,
}

impl Default for Workspace {
    fn default() -> Self {
        Self::seeded_default()
    }
}

impl Workspace {
    /// Default seed (mirrors `Default workspace seeds four columns` LLR):
    /// Sessions / Spans / Tool detail / Chat detail with widths 1.0 / 1.4 /
    /// 1.4 / 1.6, plus context widget visible at 15 rows.
    pub fn seeded_default() -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            columns: vec![
                Column {
                    id: "live_sessions".into(),
                    scenario_type: ScenarioType::LiveSessions,
                    title: "Sessions".into(),
                    config: HashMap::new(),
                    width: 1.0,
                },
                Column {
                    id: "spans".into(),
                    scenario_type: ScenarioType::Spans,
                    title: "Spans".into(),
                    config: HashMap::new(),
                    width: 1.4,
                },
                Column {
                    id: "tool_detail".into(),
                    scenario_type: ScenarioType::ToolDetail,
                    title: "Tool detail".into(),
                    config: HashMap::new(),
                    width: 1.4,
                },
                Column {
                    id: "chat_detail".into(),
                    scenario_type: ScenarioType::ChatDetail,
                    title: "Chat detail".into(),
                    config: HashMap::new(),
                    width: 1.6,
                },
            ],
            context_widget_height_rows: 15,
            context_widget_visible: true,
        }
    }

    /// Append a column with default settings for the given scenario type.
    pub fn add_column(&mut self, scenario_type: ScenarioType) {
        let id = format!(
            "{}-{}",
            scenario_type.default_title().to_lowercase().replace(' ', "_"),
            self.columns.len() + 1
        );
        self.columns.push(Column {
            id,
            scenario_type,
            title: scenario_type.default_title().into(),
            config: HashMap::new(),
            width: 1.0,
        });
    }

    pub fn remove_column(&mut self, index: usize) {
        if index < self.columns.len() {
            self.columns.remove(index);
        }
    }

    pub fn update_column<F: FnOnce(&mut Column)>(&mut self, index: usize, f: F) {
        if let Some(c) = self.columns.get_mut(index) {
            f(c);
        }
    }

    /// Move a column by signed offset. Out-of-bounds destinations are
    /// clamped to the valid range.
    pub fn move_column(&mut self, from: usize, delta: i32) {
        if from >= self.columns.len() {
            return;
        }
        let to = ((from as i32) + delta).max(0).min(self.columns.len() as i32 - 1)
            as usize;
        if from == to {
            return;
        }
        let col = self.columns.remove(from);
        self.columns.insert(to, col);
    }

    pub fn reset_default(&mut self) {
        *self = Self::seeded_default();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_seed_four_columns_in_order() {
        let w = Workspace::seeded_default();
        assert_eq!(w.columns.len(), 4);
        assert_eq!(w.columns[0].scenario_type, ScenarioType::LiveSessions);
        assert_eq!(w.columns[1].scenario_type, ScenarioType::Spans);
        assert_eq!(w.columns[2].scenario_type, ScenarioType::ToolDetail);
        assert_eq!(w.columns[3].scenario_type, ScenarioType::ChatDetail);
        assert_eq!(w.columns[0].width, 1.0);
        assert_eq!(w.columns[1].width, 1.4);
        assert_eq!(w.columns[2].width, 1.4);
        assert_eq!(w.columns[3].width, 1.6);
        assert!(w.context_widget_visible);
        assert_eq!(w.context_widget_height_rows, 15);
    }

    #[test]
    fn add_remove_move_round_trip() {
        let mut w = Workspace::seeded_default();
        w.add_column(ScenarioType::FileTouches);
        assert_eq!(w.columns.len(), 5);
        assert_eq!(w.columns[4].scenario_type, ScenarioType::FileTouches);
        w.move_column(4, -2);
        assert_eq!(w.columns[2].scenario_type, ScenarioType::FileTouches);
        w.remove_column(2);
        assert_eq!(w.columns.len(), 4);
    }
}
