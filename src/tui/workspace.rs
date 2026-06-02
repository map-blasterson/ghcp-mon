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
    /// Monotonic counter used to mint per-column IDs in [`Self::add_column`].
    /// Persisted so reloads continue from where we left off — preventing
    /// reuse of an ID that was previously freed by [`Self::remove_column`]
    /// (which would silently bind the new column to stale per-scenario
    /// state in the App's per-column HashMaps).
    #[serde(default)]
    pub next_column_id: u64,
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
            // Seeded defaults reserve IDs `live_sessions`, `spans`,
            // `tool_detail`, `chat_detail`; further `add_column` calls mint
            // unique IDs from this counter so they never collide.
            next_column_id: 1,
        }
    }

    /// Append a column with default settings for the given scenario type.
    /// The ID is minted from a monotonic counter and persisted, so removing
    /// a column and adding another of the same scenario type does NOT reuse
    /// the freed ID (which would inherit stale per-scenario state).
    pub fn add_column(&mut self, scenario_type: ScenarioType) {
        let n = self.next_column_id;
        self.next_column_id = self.next_column_id.wrapping_add(1);
        let id = format!(
            "{}-{}",
            scenario_type.default_title().to_lowercase().replace(' ', "_"),
            n
        );
        self.columns.push(Column {
            id,
            scenario_type,
            title: scenario_type.default_title().into(),
            config: HashMap::new(),
            width: 1.0,
        });
    }

    /// Remove the column at `index`. Returns the removed column's `id` so
    /// the caller can scrub per-column state held outside the workspace
    /// (per-scenario HashMaps on App). Returns `None` when out of bounds.
    pub fn remove_column(&mut self, index: usize) -> Option<String> {
        if index < self.columns.len() {
            Some(self.columns.remove(index).id)
        } else {
            None
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
        let removed = w.remove_column(2);
        assert_eq!(w.columns.len(), 4);
        assert_eq!(removed.as_deref(), Some("file_touches-1"));
    }

    /// Regression: add → remove → add of the same scenario type MUST mint
    /// a fresh ID, never reuse the freed one. Without this, the new
    /// column would inherit any per-scenario state in App's HashMaps that
    /// is keyed by the old column id.
    #[test]
    fn add_after_remove_does_not_reuse_id() {
        let mut w = Workspace::seeded_default();
        w.add_column(ScenarioType::FileTouches);
        let first_id = w.columns.last().unwrap().id.clone();
        let _ = w.remove_column(w.columns.len() - 1);
        w.add_column(ScenarioType::FileTouches);
        let second_id = w.columns.last().unwrap().id.clone();
        assert_ne!(first_id, second_id, "removed id MUST NOT be reused");
    }
}
