//! TOML load/save for [`Workspace`] under `dirs::cache_dir()/ghcp-mon/`.
//!
//! On load, the `migrate` step drops any persisted column whose
//! `scenarioType` is in the obsolete set (per `Workspace migration drops
//! obsolete scenario types`).

use anyhow::Result;
use std::path::PathBuf;

use crate::tui::workspace::{Workspace, SCHEMA_VERSION};

/// Scenario-type strings dropped during migrate (mirror the web side).
pub const OBSOLETE_SCENARIO_TYPES: &[&str] = &[
    "context_growth",
    "tool_registry",
    "context_inspector",
    "shell_io",
];

/// `dirs::cache_dir()/ghcp-mon/tui-workspace.toml`.
pub fn workspace_path() -> Option<PathBuf> {
    let dir = dirs::cache_dir()?.join("ghcp-mon");
    Some(dir.join("tui-workspace.toml"))
}

/// `dirs::cache_dir()/ghcp-mon/tui.log` — the rolling-file sink path.
pub fn log_path() -> Option<PathBuf> {
    let dir = dirs::cache_dir()?.join("ghcp-mon");
    Some(dir.join("tui.log"))
}

/// Load and migrate. Returns the seeded default on absence or parse failure.
pub fn load() -> Workspace {
    let Some(path) = workspace_path() else {
        return Workspace::default();
    };
    let Ok(text) = std::fs::read_to_string(&path) else {
        return Workspace::default();
    };
    match migrate_from_str(&text) {
        Ok(ws) => ws,
        Err(_) => Workspace::default(),
    }
}

/// Parse + drop-obsolete migrate. Returns Err if the input isn't even a
/// well-formed TOML table.
pub fn migrate_from_str(text: &str) -> Result<Workspace> {
    let mut value: toml::Value = toml::from_str(text)?;
    if let toml::Value::Table(table) = &mut value {
        if let Some(toml::Value::Array(cols)) = table.get_mut("columns") {
            cols.retain(|col| {
                let Some(t) = col.get("scenario_type").and_then(|v| v.as_str()) else {
                    return false;
                };
                !OBSOLETE_SCENARIO_TYPES.contains(&t)
            });
        }
        // Force schema_version up so the next save reflects current.
        table.insert(
            "schema_version".into(),
            toml::Value::Integer(SCHEMA_VERSION as i64),
        );
    }
    let ws: Workspace = value.try_into()?;
    Ok(ws)
}

/// Persist. Creates parent dir if missing. Errors are swallowed by callers
/// that just want best-effort persistence.
pub fn save(ws: &Workspace) -> Result<()> {
    let Some(path) = workspace_path() else {
        return Ok(());
    };
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let text = toml::to_string_pretty(ws)?;
    std::fs::write(&path, text)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::workspace::ScenarioType;

    #[test]
    fn round_trip_default() {
        let ws = Workspace::seeded_default();
        let text = toml::to_string_pretty(&ws).unwrap();
        let parsed = migrate_from_str(&text).unwrap();
        assert_eq!(parsed.columns.len(), 4);
        assert_eq!(parsed.columns[0].scenario_type, ScenarioType::LiveSessions);
    }

    #[test]
    fn migrate_drops_obsolete_columns() {
        let text = r#"
schema_version = 5
context_widget_height_rows = 15
context_widget_visible = true

[[columns]]
id = "a"
scenario_type = "context_growth"
title = "ctx"
width = 1.0

[[columns]]
id = "b"
scenario_type = "spans"
title = "Spans"
width = 1.4

[[columns]]
id = "c"
scenario_type = "tool_registry"
title = "reg"
width = 1.0
"#;
        let ws = migrate_from_str(text).unwrap();
        assert_eq!(ws.columns.len(), 1);
        assert_eq!(ws.columns[0].scenario_type, ScenarioType::Spans);
        assert_eq!(ws.schema_version, SCHEMA_VERSION);
    }
}
