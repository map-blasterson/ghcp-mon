//! Top-level Spans scenario module.
//!
//! Submodules:
//! - [`reveal_schedule`] — `Spans batch arrival smoothing` (pure scheduler).
//! - [`node_map`] — `Spans nodeMap provides O1 span lookup`.
//! - [`chips`] — shell-command / skill / report-intent / description /
//!   diff-stat extractors.
//! - [`follow_mode`] — `Spans follows latest tool span`.
//! - [`follow_chat`] — `Spans execute_tool selection auto-advances chat detail`.
//! - [`invoke_agent`] — `Spans invoke_agent selection routes to latest chat descendant`.

use std::collections::HashSet;

use crate::tui::model::{KindClass, SpanNode};
use crate::tui::scenarios::spans::reveal_schedule::RevealState;
use crate::tui::widgets::search_input::SearchInput;
use crate::tui::widgets::select::SelectState;
use crate::tui::workspace::{Column, ScenarioType};

pub mod attrs;
pub mod chips;
pub mod follow_chat;
pub mod follow_mode;
pub mod invoke_agent;
pub mod node_map;
pub mod reveal_schedule;
pub mod scenario;

pub use scenario::SpansScenario;

/// Which popover (if any) is currently overlaid on a Spans column.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpansPopover {
    Session,
    Kind,
}

/// Per-column Spans state held by App.
#[derive(Debug, Default)]
pub struct SpansState {
    pub reveal: RevealState,
    pub cursor: usize,
    pub user_collapsed: HashSet<String>,
    pub follow_mode: bool,
    pub search: SearchInput,
    pub search_active: bool,
    pub last_search_emitted: String,
    /// Server-side search hit set; `None` when no search is active.
    pub search_hits: Option<HashSet<String>>,
    /// Per-search debounce nonce — the spawned debouncer aborts when it
    /// does not match the current value at flush time.
    pub search_nonce: u64,
    pub session_picker: SelectState,
    pub kind_picker: SelectState,
    pub popover: Option<SpansPopover>,
    /// Cursor for no-session traces-list mode.
    pub traces_cursor: usize,
    /// Picked span id used to drive the span detail inspector pane.
    pub focused_span_id: Option<String>,
    /// Current session for which reveal state applies. Used to detect session
    /// switch and reset.
    pub current_session: Option<String>,
}

impl SpansState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Reset state when the column's session changes (rule 5 of
    /// `Spans batch arrival smoothing`).
    pub fn on_session_switch(&mut self, new_session: Option<&str>) {
        if self.current_session.as_deref() != new_session {
            self.reveal = reveal_schedule::reset();
            self.cursor = 0;
            self.user_collapsed.clear();
            self.current_session = new_session.map(str::to_string);
            self.search.clear();
            self.last_search_emitted.clear();
        }
    }
}

/// Per `Span selection routes by kind class allow list`, return the set of
/// scenario types that should receive a new selection for a span of `kind`.
pub fn selection_allow(kind: KindClass) -> &'static [ScenarioType] {
    // map: spans => "*", tool_detail => [execute_tool, external_tool],
    // chat_detail => [chat]
    match kind {
        KindClass::ExecuteTool | KindClass::ExternalTool => &[
            ScenarioType::Spans,
            ScenarioType::ToolDetail,
        ],
        KindClass::Chat => &[ScenarioType::Spans, ScenarioType::ChatDetail],
        KindClass::InvokeAgent => &[ScenarioType::Spans, ScenarioType::ChatDetail],
        KindClass::Other => &[ScenarioType::Spans],
    }
}

/// Selection update — what to write into a target column's config.
#[derive(Debug, Clone)]
pub struct SelectionPatch {
    pub trace_id: String,
    pub span_id: String,
    pub tool_call_id: Option<String>,
}

/// Apply `Span selection routes by kind class allow list` +
/// `Spans direct chat selection clears tool call hint` +
/// `Spans execute_tool selection auto-advances chat detail` +
/// `Spans invoke_agent selection routes to latest chat descendant`.
///
/// `picked_kind` is the picked span's `kind_class`. `picked` carries the
/// values that propagate to scenarios listed by [`selection_allow`].
/// `chat_route` is the optional `(trace_id, span_id)` for chat-detail when
/// the picked kind is `execute_tool`/`external_tool` (auto-advance) or
/// `invoke_agent` (latest chat descendant).
pub fn propagate_selection(
    columns: &mut [Column],
    picked_kind: KindClass,
    picked: SelectionPatch,
    chat_route: Option<SelectionPatch>,
    origin_idx: usize,
) {
    let allow = selection_allow(picked_kind);
    for (i, c) in columns.iter_mut().enumerate() {
        let is_origin = i == origin_idx;
        if c.scenario_type == ScenarioType::ChatDetail {
            // Chat-detail column has special routing:
            // - direct chat pick: write picked, clear tool_call_id (LLR).
            // - execute_tool/external_tool pick: write chat_route iff present.
            // - invoke_agent pick: write chat_route iff present.
            // - other kinds: leave unchanged.
            if matches!(picked_kind, KindClass::Chat) {
                c.config.insert(
                    "selected_trace_id".into(),
                    toml::Value::String(picked.trace_id.clone()),
                );
                c.config.insert(
                    "selected_span_id".into(),
                    toml::Value::String(picked.span_id.clone()),
                );
                // Spans direct chat selection clears tool call hint
                c.config.remove("selected_tool_call_id");
                continue;
            }
            if matches!(
                picked_kind,
                KindClass::ExecuteTool | KindClass::ExternalTool | KindClass::InvokeAgent
            ) {
                if let Some(p) = &chat_route {
                    c.config.insert(
                        "selected_trace_id".into(),
                        toml::Value::String(p.trace_id.clone()),
                    );
                    c.config.insert(
                        "selected_span_id".into(),
                        toml::Value::String(p.span_id.clone()),
                    );
                    // For execute_tool: also write the picked tool_call_id so
                    // the chat-detail tool-call hint can light up.
                    if matches!(
                        picked_kind,
                        KindClass::ExecuteTool | KindClass::ExternalTool
                    ) {
                        if let Some(tc) = &picked.tool_call_id {
                            c.config.insert(
                                "selected_tool_call_id".into(),
                                toml::Value::String(tc.clone()),
                            );
                        } else {
                            c.config.remove("selected_tool_call_id");
                        }
                    } else {
                        c.config.remove("selected_tool_call_id");
                    }
                }
                continue;
            }
        }
        // Generic allow-list path (Spans / ToolDetail).
        if !is_origin && !allow.contains(&c.scenario_type) {
            continue;
        }
        c.config.insert(
            "selected_trace_id".into(),
            toml::Value::String(picked.trace_id.clone()),
        );
        c.config.insert(
            "selected_span_id".into(),
            toml::Value::String(picked.span_id.clone()),
        );
        if let Some(tc) = &picked.tool_call_id {
            c.config.insert(
                "selected_tool_call_id".into(),
                toml::Value::String(tc.clone()),
            );
        }
    }
}

/// `Spans search propagates query to detail columns`. Writes `search_query`
/// into every `chat_detail`/`tool_detail` column. Pass `""` to clear.
pub fn propagate_search(columns: &mut [Column], query: &str) {
    for c in columns.iter_mut() {
        if matches!(
            c.scenario_type,
            ScenarioType::ChatDetail | ScenarioType::ToolDetail
        ) {
            c.config.insert(
                "search_query".into(),
                toml::Value::String(query.to_string()),
            );
        }
    }
}

/// `TUI Spans focused row publishes hovered chat ancestor` — find the
/// nearest chat ancestor for the focused span, returning its `span_pk` or
/// `None`. Returns the row itself's `span_pk` when it is a chat row.
pub fn hovered_chat_ancestor(tree: &[SpanNode], focused_span_id: &str) -> Option<i64> {
    let path = find_path(tree, focused_span_id)?;
    for n in path.iter().rev() {
        if matches!(n.kind_class, KindClass::Chat) {
            return Some(n.span_pk);
        }
    }
    None
}

fn find_path<'a>(tree: &'a [SpanNode], id: &str) -> Option<Vec<&'a SpanNode>> {
    fn walk<'a>(n: &'a SpanNode, id: &str, path: &mut Vec<&'a SpanNode>) -> bool {
        path.push(n);
        if n.span_id == id {
            return true;
        }
        for c in &n.children {
            if walk(c, id, path) {
                return true;
            }
        }
        path.pop();
        false
    }
    for r in tree {
        let mut path = Vec::new();
        if walk(r, id, &mut path) {
            return Some(path);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::model::SpanProjection;
    use std::collections::HashMap;

    fn mk_col(t: ScenarioType) -> Column {
        Column {
            id: format!("{:?}", t),
            scenario_type: t,
            title: format!("{:?}", t),
            config: HashMap::new(),
            width: 1.0,
        }
    }

    fn mk(id: &str, kind: KindClass, children: Vec<SpanNode>) -> SpanNode {
        SpanNode {
            span_pk: 0,
            trace_id: "t".into(),
            span_id: id.into(),
            parent_span_id: None,
            name: id.into(),
            kind_class: kind,
            ingestion_state: "complete".into(),
            start_unix_ns: None,
            end_unix_ns: None,
            projection: SpanProjection::default(),
            children,
        }
    }

    #[test]
    fn direct_chat_clears_tool_call_id() {
        let mut cols = vec![
            mk_col(ScenarioType::Spans),
            mk_col(ScenarioType::ChatDetail),
        ];
        cols[1].config.insert(
            "selected_tool_call_id".into(),
            toml::Value::String("stale".into()),
        );
        propagate_selection(
            &mut cols,
            KindClass::Chat,
            SelectionPatch {
                trace_id: "t".into(),
                span_id: "c".into(),
                tool_call_id: None,
            },
            None,
            0,
        );
        assert!(cols[1].config.get("selected_tool_call_id").is_none());
        assert_eq!(
            cols[1].config.get("selected_span_id").unwrap().as_str(),
            Some("c")
        );
    }

    #[test]
    fn execute_tool_routes_chat_via_chat_route() {
        let mut cols = vec![
            mk_col(ScenarioType::Spans),
            mk_col(ScenarioType::ToolDetail),
            mk_col(ScenarioType::ChatDetail),
        ];
        propagate_selection(
            &mut cols,
            KindClass::ExecuteTool,
            SelectionPatch {
                trace_id: "t".into(),
                span_id: "tool".into(),
                tool_call_id: Some("call-1".into()),
            },
            Some(SelectionPatch {
                trace_id: "t".into(),
                span_id: "chat".into(),
                tool_call_id: None,
            }),
            0,
        );
        // ToolDetail receives the picked span
        assert_eq!(
            cols[1].config.get("selected_span_id").unwrap().as_str(),
            Some("tool")
        );
        assert_eq!(
            cols[1].config.get("selected_tool_call_id").unwrap().as_str(),
            Some("call-1")
        );
        // ChatDetail receives the auto-advanced chat + the tool_call_id
        assert_eq!(
            cols[2].config.get("selected_span_id").unwrap().as_str(),
            Some("chat")
        );
        assert_eq!(
            cols[2].config.get("selected_tool_call_id").unwrap().as_str(),
            Some("call-1")
        );
    }

    #[test]
    fn hovered_chat_ancestor_finds_self_or_ancestor() {
        let tree = vec![mk(
            "chat",
            KindClass::Chat,
            vec![mk("tool", KindClass::ExecuteTool, vec![])],
        )];
        // self when row is chat
        let mut t = tree.clone();
        t[0].span_pk = 99;
        assert_eq!(hovered_chat_ancestor(&t, "chat"), Some(99));
        // ancestor when row is tool
        assert_eq!(hovered_chat_ancestor(&t, "tool"), Some(99));
        // None when no chat in path
        let tree2 = vec![mk("x", KindClass::Other, vec![])];
        assert_eq!(hovered_chat_ancestor(&tree2, "x"), None);
    }
}
