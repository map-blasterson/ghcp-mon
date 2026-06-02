//! `live_sessions` scenario — implements the 4 LLRs under the
//! `Live Session Browser` HLR.
//!
//! - `Live sessions list summary stats`
//! - `Selecting session propagates to dependent columns`
//! - `Delete session confirms and clears column session`
//! - `Live sessions invalidation on session and chat turn events`
//!
//! Renders a list of recent sessions; the focused row drives selection.
//! Cross-column mutations are deferred to the caller via
//! [`propagate_session`] and [`clear_session_everywhere`] — pure helpers
//! the App calls after handling key input.

use crate::tui::model::SessionSummary;
use crate::tui::workspace::{Column, ScenarioType};
use crate::tui::format::fmt_relative;

#[derive(Debug, Default, Clone)]
pub struct LiveSessionsState {
    pub cursor: usize,
}

impl LiveSessionsState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn move_cursor(&mut self, delta: i32, max: usize) {
        if max == 0 {
            self.cursor = 0;
            return;
        }
        let n = max as i32;
        let cur = self.cursor as i32;
        self.cursor = ((cur + delta).rem_euclid(n)) as usize;
    }

    pub fn jump_top(&mut self) {
        self.cursor = 0;
    }
    pub fn jump_bottom(&mut self, max: usize) {
        self.cursor = max.saturating_sub(1);
    }
}

/// One row's display text.
pub fn render_row(s: &SessionSummary) -> String {
    let id8 = s.conversation_id.chars().take(8).collect::<String>();
    let when = fmt_relative(s.last_seen_ns.map(|n| n as i128), None);
    let model = s.latest_model.as_deref().unwrap_or("—");
    let turns = pluralize(s.chat_turn_count, "turn");
    let tools = pluralize(s.tool_call_count, "tool call");
    let agents = pluralize(s.agent_run_count, "agent");
    format!("{id8}  {when}  {model}  {turns} / {tools} / {agents}")
}

fn pluralize(n: i64, singular: &str) -> String {
    if n == 1 {
        format!("1 {singular}")
    } else {
        format!("{n} {singular}s")
    }
}

/// `Selecting session propagates to dependent columns`. Sets `config.session`
/// on every column whose `scenario_type` is in the propagation set, plus the
/// originating column. Returns the number of columns mutated.
pub fn propagate_session(columns: &mut [Column], cid: &str, origin_idx: usize) -> usize {
    let mut n = 0;
    for (i, c) in columns.iter_mut().enumerate() {
        let should = i == origin_idx
            || matches!(
                c.scenario_type,
                ScenarioType::Spans | ScenarioType::ChatDetail | ScenarioType::FileTouches
            );
        if should {
            c.config.insert("session".into(), toml::Value::String(cid.to_string()));
            n += 1;
        }
    }
    n
}

/// `Delete session confirms and clears column session`. Clears
/// `config.session` on every column whose value equals `cid`.
pub fn clear_session_everywhere(columns: &mut [Column], cid: &str) -> usize {
    let mut n = 0;
    for c in columns.iter_mut() {
        let matches = c
            .config
            .get("session")
            .and_then(|v| v.as_str())
            .map(|s| s == cid)
            .unwrap_or(false);
        if matches {
            c.config.remove("session");
            // Also clear dependent selections so detail columns don't keep
            // pointing at a now-404'd span.
            c.config.remove("selected_trace_id");
            c.config.remove("selected_span_id");
            c.config.remove("selected_tool_call_id");
            n += 1;
        }
    }
    n
}

/// Build the delete confirm prompt per the LLR.
pub fn delete_prompt(cid: &str) -> (String, String) {
    let id8: String = cid.chars().take(8).collect();
    (
        "Delete session?".to_string(),
        format!("Delete session {id8}? This removes all spans, turns, and tool calls in its trace(s)."),
    )
}

/// Handle one key event in a Live Sessions column. Pure: takes the
/// scenario state, the cached sessions list, and the originating column
/// index; returns whether the key was consumed plus a list of
/// [`crate::tui::scenarios::ScenarioEffect`]s for [`crate::tui::app::App`]
/// to apply after the borrow ends.
pub fn handle_key(
    k: ratatui::crossterm::event::KeyEvent,
    state: &mut LiveSessionsState,
    sessions: &[SessionSummary],
    origin_col_idx: usize,
) -> (bool, Vec<crate::tui::scenarios::ScenarioEffect>) {
    use crate::tui::scenarios::ScenarioEffect;
    use ratatui::crossterm::event::KeyCode;
    let max = sessions.len();
    let select_current = |state: &LiveSessionsState| {
        let mut effects = Vec::new();
        if let Some(s) = sessions.get(state.cursor) {
            effects.push(ScenarioEffect::PropagateSession {
                origin_col_idx,
                cid: s.conversation_id.clone(),
            });
            effects.push(ScenarioEffect::PersistWorkspace);
        }
        effects
    };
    match k.code {
        KeyCode::Down => {
            state.move_cursor(1, max);
            (true, select_current(state))
        }
        KeyCode::Up => {
            state.move_cursor(-1, max);
            (true, select_current(state))
        }
        KeyCode::Home => {
            state.jump_top();
            (true, select_current(state))
        }
        KeyCode::End => {
            state.jump_bottom(max);
            (true, select_current(state))
        }
        KeyCode::Enter => (true, select_current(state)),
        KeyCode::Char('d') | KeyCode::Delete => {
            let mut effects = Vec::new();
            if let Some(s) = sessions.get(state.cursor) {
                effects.push(ScenarioEffect::ConfirmDeleteSession {
                    cid: s.conversation_id.clone(),
                });
            }
            (true, effects)
        }
        _ => (false, vec![]),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn mk_col(t: ScenarioType, session: Option<&str>) -> Column {
        let mut cfg = HashMap::new();
        if let Some(s) = session {
            cfg.insert("session".into(), toml::Value::String(s.into()));
        }
        Column {
            id: format!("{:?}", t),
            scenario_type: t,
            title: format!("{:?}", t),
            config: cfg,
            width: 1.0,
        }
    }

    #[test]
    fn propagate_only_hits_allowed_scenario_types() {
        let mut cols = vec![
            mk_col(ScenarioType::LiveSessions, None),
            mk_col(ScenarioType::Spans, None),
            mk_col(ScenarioType::ToolDetail, None),
            mk_col(ScenarioType::ChatDetail, None),
            mk_col(ScenarioType::FileTouches, None),
            mk_col(ScenarioType::RawBrowser, None),
        ];
        let n = propagate_session(&mut cols, "abc", 0);
        // origin (0) + Spans + ChatDetail + FileTouches = 4
        assert_eq!(n, 4);
        assert_eq!(cols[0].config.get("session").unwrap().as_str(), Some("abc"));
        assert_eq!(cols[1].config.get("session").unwrap().as_str(), Some("abc"));
        assert!(cols[2].config.get("session").is_none()); // ToolDetail untouched
        assert_eq!(cols[3].config.get("session").unwrap().as_str(), Some("abc"));
        assert_eq!(cols[4].config.get("session").unwrap().as_str(), Some("abc"));
        assert!(cols[5].config.get("session").is_none()); // Raw untouched
    }

    #[test]
    fn clear_session_only_matches_equal_cids() {
        let mut cols = vec![
            mk_col(ScenarioType::Spans, Some("abc")),
            mk_col(ScenarioType::ChatDetail, Some("xyz")),
            mk_col(ScenarioType::FileTouches, Some("abc")),
        ];
        let n = clear_session_everywhere(&mut cols, "abc");
        assert_eq!(n, 2);
        assert!(cols[0].config.get("session").is_none());
        assert_eq!(cols[1].config.get("session").unwrap().as_str(), Some("xyz"));
        assert!(cols[2].config.get("session").is_none());
    }

    #[test]
    fn pluralize_singular_and_plural() {
        assert_eq!(pluralize(0, "turn"), "0 turns");
        assert_eq!(pluralize(1, "turn"), "1 turn");
        assert_eq!(pluralize(3, "turn"), "3 turns");
    }

    #[test]
    fn render_row_includes_id_prefix_and_counts() {
        let s = SessionSummary {
            conversation_id: "abcdef0123456789".into(),
            first_seen_ns: None,
            last_seen_ns: None,
            latest_model: Some("gpt-x".into()),
            chat_turn_count: 3,
            tool_call_count: 1,
            agent_run_count: 0,
            service_name: None,
            local_name: None,
            user_named: None,
            cwd: None,
            branch: None,
        };
        let row = render_row(&s);
        assert!(row.contains("abcdef01"));
        assert!(row.contains("gpt-x"));
        assert!(row.contains("3 turns"));
        assert!(row.contains("1 tool call"));
        assert!(row.contains("0 agents"));
    }

    fn mk_session(cid: &str) -> SessionSummary {
        SessionSummary {
            conversation_id: cid.into(),
            first_seen_ns: None,
            last_seen_ns: None,
            latest_model: None,
            chat_turn_count: 0,
            tool_call_count: 0,
            agent_run_count: 0,
            service_name: None,
            local_name: None,
            user_named: None,
            cwd: None,
            branch: None,
        }
    }

    #[test]
    fn down_moves_cursor_and_live_selects_session() {
        use crate::tui::scenarios::ScenarioEffect;
        use ratatui::crossterm::event::KeyCode;

        let sessions = vec![mk_session("cid-a"), mk_session("cid-b")];
        let mut state = LiveSessionsState::default();
        let (consumed, effects) = handle_key(
            ratatui::crossterm::event::KeyEvent::from(KeyCode::Down),
            &mut state,
            &sessions,
            0,
        );

        assert!(consumed);
        assert_eq!(state.cursor, 1);
        assert!(matches!(
            effects.as_slice(),
            [ScenarioEffect::PropagateSession { cid, .. }, ScenarioEffect::PersistWorkspace]
                if cid == "cid-b"
        ));

        let mut cols = vec![
            mk_col(ScenarioType::LiveSessions, None),
            mk_col(ScenarioType::Spans, None),
            mk_col(ScenarioType::ChatDetail, None),
        ];
        if let ScenarioEffect::PropagateSession { origin_col_idx, cid } = &effects[0] {
            propagate_session(&mut cols, cid, *origin_col_idx);
        }
        assert_eq!(cols[0].config.get("session").unwrap().as_str(), Some("cid-b"));
        assert_eq!(cols[1].config.get("session").unwrap().as_str(), Some("cid-b"));
        assert_eq!(cols[2].config.get("session").unwrap().as_str(), Some("cid-b"));
    }

    #[test]
    fn delete_prompt_uses_first_8_chars() {
        let (title, body) = delete_prompt("abcdef0123456789");
        assert_eq!(title, "Delete session?");
        assert!(body.contains("abcdef01"));
        assert!(body.contains("removes all spans"));
    }
}
