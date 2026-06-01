//! Render + behavior tests for the tool-detail scenario.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Color;
use serde_json::{json, Value};

use super::*;
use crate::tui::model::SpanDetail;

// ---- fixtures --------------------------------------------------------------

fn mk_span(attrs: Value) -> Value {
    json!({
        "span_pk": 1,
        "trace_id": "t1",
        "span_id": "s1",
        "parent_span_id": null,
        "name": "execute_tool",
        "kind": null,
        "kind_class": "other",
        "start_unix_ns": 1_700_000_000_000_000_000i64,
        "end_unix_ns": 1_700_000_000_015_000_000i64,
        "duration_ns": 15_000_000,
        "status_message": null,
        "ingestion_state": "complete",
        "scope_name": null,
        "scope_version": null,
        "attributes": attrs,
        "resource": null
    })
}

fn native_detail(tool_name: &str, tool_type: &str, attrs: Value) -> SpanDetail {
    let v = json!({
        "span": mk_span(attrs),
        "events": [],
        "parent": null,
        "children": [],
        "projection": {
            "tool_call": {
                "tool_call_pk": 7,
                "call_id": "call_123",
                "tool_name": tool_name,
                "tool_type": tool_type,
                "conversation_id": "conv-abcdef0123",
                "agent_run_pk": null,
                "status_code": 200
            }
        }
    });
    serde_json::from_value(v).expect("SpanDetail")
}

fn external_detail(attrs: Value) -> SpanDetail {
    let v = json!({
        "span": mk_span(attrs),
        "events": [],
        "parent": null,
        "children": [],
        "projection": {
            "external_tool_call": {
                "ext_pk": 3,
                "call_id": "ext_call",
                "tool_name": "mcp_search",
                "paired_tool_call_pk": 42,
                "conversation_id": "conv-zzz999888",
                "agent_run_pk": 9
            }
        }
    });
    serde_json::from_value(v).expect("SpanDetail")
}

fn not_a_tool_detail() -> SpanDetail {
    let v = json!({
        "span": mk_span(Value::Null),
        "events": [],
        "parent": null,
        "children": [],
        "projection": {}
    });
    serde_json::from_value(v).expect("SpanDetail")
}

// ---- buffer helpers --------------------------------------------------------

fn row_text(buf: &Buffer, area: Rect, y: u16) -> String {
    (area.x..area.x + area.width)
        .map(|x| buf[(x, y)].symbol().to_string())
        .collect()
}

fn full_text(buf: &Buffer, area: Rect) -> String {
    (area.y..area.y + area.height)
        .map(|y| row_text(buf, area, y))
        .collect::<Vec<_>>()
        .join("\n")
}

fn any_cell_with_bg(buf: &Buffer, area: Rect, bg: Color) -> bool {
    (area.y..area.y + area.height)
        .any(|y| (area.x..area.x + area.width).any(|x| buf[(x, y)].bg == bg))
}

fn any_styled_fg(buf: &Buffer, area: Rect) -> bool {
    (area.y..area.y + area.height).any(|y| {
        (area.x..area.x + area.width)
            .any(|x| buf[(x, y)].fg != Color::Reset && buf[(x, y)].symbol() != " ")
    })
}

fn render_to(
    state: &mut ToolDetailState,
    selection: Option<(&str, &str)>,
    search_query: Option<&str>,
    detail: Option<&SpanDetail>,
    focused: bool,
) -> (Buffer, Rect) {
    let area = Rect::new(0, 0, 60, 30);
    let mut buf = Buffer::empty(area);
    render(area, &mut buf, state, selection, search_query, detail, focused);
    (buf, area)
}

// ---- empty states ----------------------------------------------------------

#[test]
fn empty_when_no_selection() {
    let mut st = ToolDetailState::default();
    let (buf, area) = render_to(&mut st, None, None, None, false);
    assert!(full_text(&buf, area).contains("Select a tool span"));
}

#[test]
fn loading_when_selected_but_no_detail() {
    let mut st = ToolDetailState::default();
    let (buf, area) = render_to(&mut st, Some(("t1", "s1")), None, None, false);
    assert!(full_text(&buf, area).contains("loading"));
}

#[test]
fn not_a_tool_state_verbatim() {
    let mut st = ToolDetailState::default();
    let d = not_a_tool_detail();
    let (buf, area) = render_to(&mut st, Some(("t1", "s1")), None, Some(&d), false);
    assert!(full_text(&buf, area).contains(NOT_A_TOOL_LINE));
}

// ---- hero ------------------------------------------------------------------

#[test]
fn hero_present_for_function_command() {
    let mut st = ToolDetailState::default();
    let attrs = json!({"gen_ai.tool.call.arguments": {"command": "ls -la /tmp"}});
    let d = native_detail("bash", "function", attrs);
    let (buf, area) = render_to(&mut st, Some(("t1", "s1")), None, Some(&d), false);
    assert!(full_text(&buf, area).contains("ls -la /tmp"));
}

#[test]
fn hero_suppressed_for_non_function() {
    let mut st = ToolDetailState::default();
    let attrs = json!({"gen_ai.tool.call.arguments": {"command": "ls -la /tmp"}});
    let d = native_detail("bash", "builtin", attrs);
    let (buf, area) = render_to(&mut st, Some(("t1", "s1")), None, Some(&d), false);
    assert!(full_text(&buf, area).contains("bash"));
}

// ---- metadata panel --------------------------------------------------------

#[test]
fn metadata_closed_by_default_and_space_opens() {
    use ratatui::crossterm::event::{KeyCode, KeyEvent};
    let mut st = ToolDetailState::default();
    let attrs = json!({"gen_ai.tool.call.arguments": {"x": 1}});
    let d = native_detail("some_tool", "function", attrs);
    let _ = render_to(&mut st, Some(("t1", "s1")), None, Some(&d), true);
    assert!(!st.metadata_open);
    let (buf, area) = render_to(&mut st, Some(("t1", "s1")), None, Some(&d), true);
    assert!(!full_text(&buf, area).contains("call_123"));
    let consumed = handle_key(KeyEvent::from(KeyCode::Char(' ')), &mut st);
    assert!(consumed);
    assert!(st.metadata_open);
    let (buf, area) = render_to(&mut st, Some(("t1", "s1")), None, Some(&d), true);
    assert!(full_text(&buf, area).contains("call_123"));
}

// ---- edit renderer ---------------------------------------------------------

#[test]
fn edit_old_new_syntax_highlight_cell() {
    let mut st = ToolDetailState::default();
    let attrs = json!({"gen_ai.tool.call.arguments": {
        "path": "main.rs",
        "old_str": "fn old() { let x = 1; }",
        "new_str": "fn new() { let y = 2; }"
    }});
    let d = native_detail("edit", "function", attrs);
    let (buf, area) = render_to(&mut st, Some(("t1", "s1")), None, Some(&d), false);
    let text = full_text(&buf, area);
    assert!(text.contains("path"));
    assert!(any_styled_fg(&buf, area), "expected syntect-styled cells");
}

#[test]
fn copilot_create_renders_file_text_as_content_block() {
    // Regression: Copilot's `create` tool uses the body-text concept
    // mapped to `args.file_text`. Previously render_edit ignored it,
    // so create spans rendered the file body as JSON under "other".
    // After fix: path + a single "content"-labelled code block with the
    // file body, syntax-highlighted by lang_from_path; "other" section
    // MUST NOT appear (no remaining args).
    let mut st = ToolDetailState::default();
    let attrs = json!({"gen_ai.tool.call.arguments": {
        "path": "main.rs",
        "file_text": "fn brand_new() { let z = 3; }"
    }});
    let d = native_detail("create", "function", attrs);
    let (buf, area) = render_to(&mut st, Some(("t1", "s1")), None, Some(&d), false);
    let text = full_text(&buf, area);
    assert!(text.contains("content"), "missing 'content' label in:\n{text}");
    assert!(text.contains("brand_new"), "missing file body in:\n{text}");
    assert!(!text.contains("\"file_text\""), "file_text leaked into 'other' JSON in:\n{text}");
    assert!(!text.contains("other"), "spurious 'other' section in:\n{text}");
    assert!(any_styled_fg(&buf, area), "expected syntect-styled cells for the content block");
}

// ---- view renderer ---------------------------------------------------------

#[test]
fn view_gutter_and_body() {
    let mut st = ToolDetailState::default();
    let attrs = json!({
        "gen_ai.tool.call.arguments": {"path": "main.rs"},
        "gen_ai.tool.call.result": "1. fn main() {\n2. }"
    });
    let d = native_detail("view", "function", attrs);
    let (buf, area) = render_to(&mut st, Some(("t1", "s1")), None, Some(&d), false);
    let text = full_text(&buf, area);
    assert!(text.contains("fn main()"));
    assert!(text.contains('1') && text.contains('2'));
}

// ---- task renderer (markdown) ----------------------------------------------

#[test]
fn task_prompt_renders_markdown_heading() {
    let mut st = ToolDetailState::default();
    let attrs = json!({"gen_ai.tool.call.arguments": {
        "prompt": "# Heading\n\nbody text"
    }});
    let d = native_detail("task", "function", attrs);
    let (buf, area) = render_to(&mut st, Some(("t1", "s1")), None, Some(&d), false);
    assert!(full_text(&buf, area).contains("Heading"));
}

// ---- generic renderer ------------------------------------------------------

#[test]
fn generic_splits_codeish_string() {
    let mut st = ToolDetailState::default();
    let attrs = json!({"gen_ai.tool.call.arguments": {
        "patch": "line1\nline2\nline3",
        "count": 3
    }});
    let d = native_detail("weird_tool", "function", attrs);
    let (buf, area) = render_to(&mut st, Some(("t1", "s1")), None, Some(&d), false);
    let text = full_text(&buf, area);
    assert!(text.contains("patch"));
    assert!(text.contains("line2"));
    assert!(text.contains("count"));
}

// ---- external body ---------------------------------------------------------

#[test]
fn external_header_fields_and_generic() {
    let mut st = ToolDetailState::default();
    let attrs = json!({"gen_ai.tool.call.arguments": {"query": "find me"}});
    let d = external_detail(attrs);
    let _ = render_to(&mut st, Some(("t1", "s1")), None, Some(&d), true);
    st.metadata_open = true;
    let (buf, area) = render_to(&mut st, Some(("t1", "s1")), None, Some(&d), true);
    let text = full_text(&buf, area);
    assert!(text.contains("mcp_search"));
    assert!(text.contains("external"));
    assert!(text.contains("paired_tool_call_pk"));
}

// ---- external search highlight ---------------------------------------------

#[test]
fn external_query_highlights_match_cell() {
    let mut st = ToolDetailState::default();
    let attrs = json!({"gen_ai.tool.call.result": "the quick brown fox"});
    let d = native_detail("weird_tool", "function", attrs);
    let (buf, area) = render_to(&mut st, Some(("t1", "s1")), Some("quick"), Some(&d), false);
    assert!(
        any_cell_with_bg(&buf, area, Color::Yellow)
            || any_cell_with_bg(&buf, area, Color::Rgb(255, 200, 0)),
        "expected a highlighted match cell"
    );
}

// ---- markdown body search highlight ----------------------------------------

#[test]
fn task_markdown_body_highlights_external_query_match() {
    let mut st = ToolDetailState::default();
    let attrs = json!({
        "gen_ai.tool.call.arguments": {"prompt": "investigate the foo module"}
    });
    let d = native_detail("task", "function", attrs);
    let (buf, area) = render_to(&mut st, Some(("t1", "s1")), Some("foo"), Some(&d), false);
    assert!(
        any_cell_with_bg(&buf, area, Color::Yellow)
            || any_cell_with_bg(&buf, area, Color::Rgb(255, 200, 0)),
        "expected a highlighted match cell in the markdown prompt body"
    );
    // The block must be searchable: it registers a focus entry.
    assert!(
        st.focus_plan.iter().any(|(k, _)| k == "task.prompt"),
        "expected task.prompt to register as a searchable focus block"
    );
}

// ---- key dispatch ----------------------------------------------------------

#[test]
fn tab_cycles_blocks_then_falls_through() {
    use ratatui::crossterm::event::{KeyCode, KeyEvent};
    let mut st = ToolDetailState::default();
    let attrs = json!({"gen_ai.tool.call.arguments": {"x": 1}});
    let d = native_detail("some_tool", "function", attrs);
    let _ = render_to(&mut st, Some(("t1", "s1")), None, Some(&d), true);
    let n = st.focus_plan.len();
    assert!(n >= 2, "expected at least metadata + json blocks");
    let mut consumed_count = 0;
    loop {
        let c = handle_key(KeyEvent::from(KeyCode::Tab), &mut st);
        if c {
            consumed_count += 1;
        } else {
            break;
        }
        if consumed_count > 10 {
            panic!("Tab never fell through");
        }
    }
    assert_eq!(consumed_count, n - 1);
    assert_eq!(st.focused_block, 0, "fell through resets to first block");
}

#[test]
fn end_then_down_scroll_clamps() {
    use ratatui::crossterm::event::{KeyCode, KeyEvent};
    let mut st = ToolDetailState::default();
    let big_body: String = (0..100).map(|i| format!("line {i}\n")).collect();
    let attrs = json!({"gen_ai.tool.call.result": big_body});
    let d = native_detail("weird_tool", "function", attrs);
    let _ = render_to(&mut st, Some(("t1", "s1")), None, Some(&d), true);
    handle_key(KeyEvent::from(KeyCode::Home), &mut st);
    assert_eq!(st.scroll_top, 0);
    handle_key(KeyEvent::from(KeyCode::End), &mut st);
    assert!(st.scroll_top > 0);
    let at_end = st.scroll_top;
    handle_key(KeyEvent::from(KeyCode::Down), &mut st);
    assert_eq!(st.scroll_top, at_end);
}
