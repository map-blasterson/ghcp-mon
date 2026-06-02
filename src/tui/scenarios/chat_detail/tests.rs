//! Rendering-level tests for the chat-detail scenario using `TestBackend`.
//! Pure-logic tests live in the per-module `#[cfg(test)]` blocks (tree,
//! diff_segments, messages, walk, search_expand, tool_call_hint).

use super::tree::ChatMode;
use super::*;
use crate::tui::model::{KindClass, SpanDetail, SpanFull, SpanProjection};
use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::Terminal;
use serde_json::{json, Value};

fn span_with_attrs(kind_class: KindClass, attrs: Option<Value>) -> SpanDetail {
    SpanDetail {
        span: SpanFull {
            span_pk: 1,
            trace_id: "t".into(),
            span_id: "s".into(),
            parent_span_id: None,
            name: "n".into(),
            kind: Some(1),
            kind_class,
            start_unix_ns: Some(1),
            end_unix_ns: Some(2),
            duration_ns: Some(1),
            status_message: None,
            ingestion_state: "real".into(),
            scope_name: None,
            scope_version: None,
            attributes: attrs,
            resource: None,
        },
        events: Vec::new(),
        parent: None,
        children: Vec::new(),
        projection: SpanProjection::default(),
    }
}

fn buf_to_string(buf: &Buffer) -> String {
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
fn empty_state_for_non_chat_span() {
    let mut term = Terminal::new(TestBackend::new(80, 10)).unwrap();
    let detail = span_with_attrs(KindClass::ExecuteTool, None);
    let mut state = ChatDetailState::default();
    term.draw(|f| {
        let area = Rect::new(0, 0, 80, 10);
        render(
            area,
            f.buffer_mut(),
            &mut state,
            Some(("t", "s")),
            None,
            None,
            Some(&detail),
            None,
            true,
        );
    })
    .unwrap();
    let s = buf_to_string(term.backend().buffer());
    assert!(s.contains("selected span is not a chat span"), "got:\n{s}");
}

#[test]
fn empty_state_when_no_selection() {
    let mut term = Terminal::new(TestBackend::new(80, 5)).unwrap();
    let mut state = ChatDetailState::default();
    term.draw(|f| {
        render(
            Rect::new(0, 0, 80, 5),
            f.buffer_mut(),
            &mut state,
            None,
            None,
            None,
            None,
            None,
            true,
        );
    })
    .unwrap();
    assert!(buf_to_string(term.backend().buffer()).contains("Select a chat span"));
}

#[test]
fn header_shows_delta_mode_chip_by_default() {
    let attrs = json!({
        "gen_ai.system_instructions": [{"type":"text","content":"hi"}],
    });
    let detail = span_with_attrs(KindClass::Chat, Some(attrs));
    let mut term = Terminal::new(TestBackend::new(80, 10)).unwrap();
    let mut state = ChatDetailState::default();
    term.draw(|f| {
        render(
            Rect::new(0, 0, 80, 10),
            f.buffer_mut(),
            &mut state,
            Some(("t", "s")),
            None,
            None,
            Some(&detail),
            None,
            true,
        );
    })
    .unwrap();
    let s = buf_to_string(term.backend().buffer());
    assert!(s.contains("[DELTA]"), "expected [DELTA] in header, got:\n{s}");
}

#[test]
fn m_key_toggles_mode() {
    let mut st = ChatDetailState::default();
    let consumed = handle_key(
        ratatui::crossterm::event::KeyEvent::new(
            ratatui::crossterm::event::KeyCode::Char('m'),
            ratatui::crossterm::event::KeyModifiers::empty(),
        ),
        &mut st,
    );
    assert!(consumed);
    assert_eq!(st.mode, ChatMode::Full);
    handle_key(
        ratatui::crossterm::event::KeyEvent::new(
            ratatui::crossterm::event::KeyCode::Char('m'),
            ratatui::crossterm::event::KeyModifiers::empty(),
        ),
        &mut st,
    );
    assert_eq!(st.mode, ChatMode::Delta);
}

#[test]
fn renders_four_branch_root() {
    let attrs = json!({
        "gen_ai.system_instructions": [{"type":"text","content":"sys"}],
        "gen_ai.tool.definitions": [{"name":"ls"}],
        "gen_ai.input.messages": [
            {"role":"user","parts":[{"type":"text","content":"q"}]}
        ],
        "gen_ai.output.messages": [
            {"role":"assistant","parts":[{"type":"text","content":"a"}]}
        ]
    });
    let detail = span_with_attrs(KindClass::Chat, Some(attrs));
    let mut term = Terminal::new(TestBackend::new(100, 12)).unwrap();
    let mut state = ChatDetailState::default();
    term.draw(|f| {
        render(
            Rect::new(0, 0, 100, 12),
            f.buffer_mut(),
            &mut state,
            Some(("t", "s")),
            None,
            None,
            Some(&detail),
            None,
            true,
        );
    })
    .unwrap();
    let s = buf_to_string(term.backend().buffer());
    for branch in [
        "system instructions",
        "tool definitions",
        "input messages",
        "output messages",
    ] {
        assert!(s.contains(branch), "missing branch {branch} in:\n{s}");
    }
}

#[test]
fn no_content_state_renders_hint() {
    let attrs = json!({});
    let detail = span_with_attrs(KindClass::Chat, Some(attrs));
    let mut term = Terminal::new(TestBackend::new(80, 6)).unwrap();
    let mut state = ChatDetailState::default();
    term.draw(|f| {
        render(
            Rect::new(0, 0, 80, 6),
            f.buffer_mut(),
            &mut state,
            Some(("t", "s")),
            None,
            None,
            Some(&detail),
            None,
            true,
        );
    })
    .unwrap();
    assert!(buf_to_string(term.backend().buffer()).contains("no content captured"));
}

#[test]
fn summary_bar_paints_distinct_colors_for_visible_segments() {
    let attrs = json!({
        "gen_ai.system_instructions": [{"type":"text","content":"sys"}],
        "gen_ai.tool.definitions": [{"name":"ls","description":"x"}],
        "gen_ai.input.messages": [
            {"role":"user","parts":[{"type":"text","content":"q"}]}
        ],
        "gen_ai.output.messages": [
            {"role":"assistant","parts":[{"type":"text","content":"a"}]}
        ]
    });
    let detail = span_with_attrs(KindClass::Chat, Some(attrs));
    let mut term = Terminal::new(TestBackend::new(80, 10)).unwrap();
    let mut state = ChatDetailState::default();
    term.draw(|f| {
        render(
            Rect::new(0, 0, 80, 10),
            f.buffer_mut(),
            &mut state,
            Some(("t", "s")),
            None,
            None,
            Some(&detail),
            None,
            true,
        );
    })
    .unwrap();
    let buf = term.backend().buffer();
    let mut colors: std::collections::HashSet<ratatui::style::Color> =
        std::collections::HashSet::new();
    for x in 0..80 {
        colors.insert(buf[(x, 1)].fg); // row 1 = summary bar
    }
    assert!(colors.len() >= 2, "expected ≥ 2 distinct colours in summary bar, got {:?}", colors);
}

#[test]
fn summary_bar_dark_shades_unchanged_segments_in_delta_mode() {
    // In DELTA mode with prior == current, system instructions and tool
    // definitions get the `Unchanged` badge and should render with a
    // DARKENED version of their fg color. Input/output messages don't carry
    // an Unchanged badge at the section level (input falls through to per-
    // turn delta; output isn't diffed) so they keep their bright fg.
    use crate::tui::widgets::summary_bar::shaded_color;
    use ratatui::style::Color;
    // From color_for_node in chat_detail/mod.rs:
    let bright_system = Color::Rgb(0x60, 0xa5, 0xfa);
    let bright_input = Color::Rgb(0x4a, 0xde, 0x80);
    let dark_system = shaded_color(bright_system);

    let attrs = json!({
        "gen_ai.system_instructions": [{"type":"text","content":"sys"}],
        "gen_ai.tool.definitions": [{"name":"ls","description":"x"}],
        "gen_ai.input.messages": [
            {"role":"user","parts":[{"type":"text","content":"q"}]}
        ],
        "gen_ai.output.messages": [
            {"role":"assistant","parts":[{"type":"text","content":"a"}]}
        ]
    });
    let detail = span_with_attrs(KindClass::Chat, Some(attrs.clone()));
    let mut term = Terminal::new(TestBackend::new(80, 10)).unwrap();
    let mut state = ChatDetailState::default(); // defaults to DELTA mode
    term.draw(|f| {
        render(
            Rect::new(0, 0, 80, 10),
            f.buffer_mut(),
            &mut state,
            Some(("t", "s")),
            None,
            None,
            Some(&detail),
            Some(&attrs), // prior == current → sys + tools unchanged
            true,
        );
    })
    .unwrap();
    let buf = term.backend().buffer();
    let mut colors: std::collections::HashSet<Color> = std::collections::HashSet::new();
    for x in 0..80 {
        // Every bar cell should be FULL block regardless of shading.
        assert_eq!(buf[(x, 1)].symbol(), "█", "bar cell should be FULL at x={x}");
        colors.insert(buf[(x, 1)].fg);
    }
    // Sys segment is the leftmost; assert it renders darkened.
    assert!(
        colors.contains(&dark_system),
        "expected darkened-system fg in bar, got {:?}",
        colors
    );
    // Bright system must NOT appear (the entire system segment is shaded).
    assert!(
        !colors.contains(&bright_system),
        "expected no bright-system fg in bar, got {:?}",
        colors
    );
    // Input segment is unchanged-at-section-level=false → bright green is
    // still present.
    assert!(
        colors.contains(&bright_input),
        "expected bright-input fg in bar, got {:?}",
        colors
    );
}

#[test]
fn summary_bar_uses_full_glyph_in_full_mode_even_with_prior() {
    // FULL mode never tags Unchanged → every segment uses its bright color.
    use crate::tui::widgets::summary_bar::shaded_color;
    use ratatui::style::Color;
    let bright_system = Color::Rgb(0x60, 0xa5, 0xfa);
    let dark_system = shaded_color(bright_system);

    let attrs = json!({
        "gen_ai.system_instructions": [{"type":"text","content":"sys"}],
        "gen_ai.tool.definitions": [{"name":"ls","description":"x"}],
        "gen_ai.input.messages": [
            {"role":"user","parts":[{"type":"text","content":"q"}]}
        ],
        "gen_ai.output.messages": [
            {"role":"assistant","parts":[{"type":"text","content":"a"}]}
        ]
    });
    let detail = span_with_attrs(KindClass::Chat, Some(attrs.clone()));
    let mut term = Terminal::new(TestBackend::new(80, 10)).unwrap();
    let mut state = ChatDetailState::default();
    state.mode = ChatMode::Full;
    term.draw(|f| {
        render(
            Rect::new(0, 0, 80, 10),
            f.buffer_mut(),
            &mut state,
            Some(("t", "s")),
            None,
            None,
            Some(&detail),
            Some(&attrs),
            true,
        );
    })
    .unwrap();
    let buf = term.backend().buffer();
    let mut colors: std::collections::HashSet<Color> = std::collections::HashSet::new();
    for x in 0..80 {
        colors.insert(buf[(x, 1)].fg);
    }
    assert!(
        colors.contains(&bright_system),
        "FULL mode must keep bright system color, got {:?}",
        colors
    );
    assert!(
        !colors.contains(&dark_system),
        "FULL mode must never produce darkened colors, got {:?}",
        colors
    );
}

/// Helper that drives a single render and returns the rendered buffer.
fn render_once(
    state: &mut ChatDetailState,
    detail: &SpanDetail,
    selected_tool_call_id: Option<&str>,
    selection: (&str, &str),
    w: u16,
    h: u16,
) -> ratatui::buffer::Buffer {
    let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
    term.draw(|f| {
        render(
            Rect::new(0, 0, w, h),
            f.buffer_mut(),
            state,
            Some(selection),
            None,
            selected_tool_call_id,
            Some(detail),
            None,
            true,
        );
    })
    .unwrap();
    term.backend().buffer().clone()
}

#[test]
fn tool_call_follow_snaps_focus_row_to_target_message() {
    // Two input messages so target is NOT index 0 — proves the snap moved
    // focus_row away from its default.
    let attrs = json!({
        "gen_ai.input.messages": [
            {"role":"user","parts":[{"type":"text","content":"hi"}]},
            {"role":"tool","parts":[
                {"type":"tool_call_response","id":"call_X","result":"ok"}
            ]}
        ]
    });
    let detail = span_with_attrs(KindClass::Chat, Some(attrs));
    let mut state = ChatDetailState::default();
    assert_eq!(state.focus_row, 0, "precondition: focus_row defaults to 0");
    let buf = render_once(&mut state, &detail, Some("call_X"), ("t", "s"), 100, 15);
    // After snap, focus_row must point at the tool-role message row.
    let target_id = NodeId::from("root/input/input_messages/1");
    let actual_id = state
        .last_focus_map
        .get(state.focus_row)
        .and_then(|o| o.as_ref())
        .cloned();
    assert_eq!(
        actual_id.as_ref(),
        Some(&target_id),
        "focus_row={} should point at tool target, got {:?}",
        state.focus_row,
        actual_id,
    );
    assert_ne!(state.focus_row, 0, "snap must have moved focus_row off 0");
    // Follow key recorded so future renders with the same selection don't
    // re-snap (verified in a separate test).
    assert_eq!(
        state.last_follow_key.as_deref(),
        Some("t|s|call_X"),
        "follow key should be recorded after successful snap",
    );
    // The unified focus marker (▶, yellow) appears in column 0 of the row
    // at the tree-area y-offset for focus_row.
    let tree_y0 = 3u16; // header + bar + indicator
    let view_h = state.last_view_h;
    let row_y = tree_y0 + (state.focus_row as u16 - state.scroll_top).min(view_h.saturating_sub(1));
    assert_eq!(buf[(0, row_y)].symbol(), "▶", "expected ▶ at the focus row col 0");
    assert_eq!(buf[(0, row_y)].fg, ratatui::style::Color::Rgb(0xfd, 0xe0, 0x47));
}

#[test]
fn tool_call_follow_does_not_re_snap_on_repeat_with_same_id() {
    let attrs = json!({
        "gen_ai.input.messages": [
            {"role":"user","parts":[{"type":"text","content":"hi"}]},
            {"role":"tool","parts":[
                {"type":"tool_call_response","id":"call_X","result":"ok"}
            ]}
        ]
    });
    let detail = span_with_attrs(KindClass::Chat, Some(attrs));
    let mut state = ChatDetailState::default();
    // First render: snap.
    let _ = render_once(&mut state, &detail, Some("call_X"), ("t", "s"), 100, 15);
    let snapped_row = state.focus_row;
    assert_ne!(snapped_row, 0);
    // User moves cursor manually.
    state.focus_row = 0;
    // Second render: same id → MUST NOT re-snap.
    let _ = render_once(&mut state, &detail, Some("call_X"), ("t", "s"), 100, 15);
    assert_eq!(
        state.focus_row, 0,
        "same tool id must not re-snap after user navigation",
    );
}

#[test]
fn tool_call_follow_re_snaps_after_clear_and_reselect() {
    let attrs = json!({
        "gen_ai.input.messages": [
            {"role":"user","parts":[{"type":"text","content":"hi"}]},
            {"role":"tool","parts":[
                {"type":"tool_call_response","id":"call_X","result":"ok"}
            ]}
        ]
    });
    let detail = span_with_attrs(KindClass::Chat, Some(attrs));
    let mut state = ChatDetailState::default();
    let _ = render_once(&mut state, &detail, Some("call_X"), ("t", "s"), 100, 15);
    let snapped_row = state.focus_row;
    assert_ne!(snapped_row, 0);
    // Clear tool selection.
    state.focus_row = 0;
    let _ = render_once(&mut state, &detail, None, ("t", "s"), 100, 15);
    assert_eq!(state.focus_row, 0, "no tool selection → no snap");
    assert_eq!(state.last_follow_key, None, "clearing resets follow key");
    // Re-select same tool.
    let _ = render_once(&mut state, &detail, Some("call_X"), ("t", "s"), 100, 15);
    assert_eq!(
        state.focus_row, snapped_row,
        "re-selecting same tool after clear must re-snap",
    );
}

#[test]
fn tool_call_follow_re_snaps_when_selected_span_changes() {
    let attrs = json!({
        "gen_ai.input.messages": [
            {"role":"user","parts":[{"type":"text","content":"hi"}]},
            {"role":"tool","parts":[
                {"type":"tool_call_response","id":"call_X","result":"ok"}
            ]}
        ]
    });
    let detail = span_with_attrs(KindClass::Chat, Some(attrs));
    let mut state = ChatDetailState::default();
    // First render in span "s1" snaps.
    let _ = render_once(&mut state, &detail, Some("call_X"), ("t", "s1"), 100, 15);
    let snapped_row = state.focus_row;
    state.focus_row = 0;
    // Same tool id but selected span changed → must re-snap because the
    // follow key is composite (trace|span|tcid).
    let _ = render_once(&mut state, &detail, Some("call_X"), ("t", "s2"), 100, 15);
    assert_eq!(
        state.focus_row, snapped_row,
        "different selected span with same tool id must re-snap",
    );
    assert_eq!(state.last_follow_key.as_deref(), Some("t|s2|call_X"));
}

#[test]
fn bar_indicator_appears_under_input_segment_in_follow_mode() {
    // When follow-snap puts focus_row on a message under input.messages, the
    // bar hover indicator (yellow ▲) should appear across the input segment
    // because the hover matcher uses slash-prefix ancestor matching and the
    // focused node id starts with "root/input/input_messages/".
    let attrs = json!({
        "gen_ai.input.messages": [
            {"role":"tool","parts":[
                {"type":"tool_call_response","id":"call_X","result":"ok"}
            ]}
        ]
    });
    let detail = span_with_attrs(KindClass::Chat, Some(attrs));
    let mut state = ChatDetailState::default();
    let buf = render_once(&mut state, &detail, Some("call_X"), ("t", "s"), 100, 15);
    // Indicator row is at y=2 (header=0, bar=1, indicator=2).
    let mut arrow_cells = 0usize;
    for x in 0..100 {
        if buf[(x, 2)].symbol() == "▲" {
            arrow_cells += 1;
            assert_eq!(buf[(x, 2)].fg, ratatui::style::Color::Yellow);
        }
    }
    assert!(
        arrow_cells > 0,
        "expected ▲ cells in indicator row when follow snap focuses an input message",
    );
}

#[test]
fn delta_system_unchanged_meta_visible_in_render() {
    let attrs = json!({
        "gen_ai.system_instructions": [{"type":"text","content":"same"}],
    });
    let prior_attrs = json!({
        "gen_ai.system_instructions": [{"type":"text","content":"same"}],
    });
    let detail = span_with_attrs(KindClass::Chat, Some(attrs));
    let mut term = Terminal::new(TestBackend::new(100, 8)).unwrap();
    let mut state = ChatDetailState::default();
    term.draw(|f| {
        render(
            Rect::new(0, 0, 100, 8),
            f.buffer_mut(),
            &mut state,
            Some(("t", "s")),
            None,
            None,
            Some(&detail),
            Some(&prior_attrs),
            true,
        );
    })
    .unwrap();
    let s = buf_to_string(term.backend().buffer());
    assert!(s.contains("unchanged"), "missing 'unchanged' in:\n{s}");
}

#[test]
fn delta_system_changed_meta_and_reversed_cells() {
    let attrs = json!({
        "gen_ai.system_instructions": [{"type":"text","content":"hello brand new"}],
    });
    let prior_attrs = json!({
        "gen_ai.system_instructions": [{"type":"text","content":"hello world"}],
    });
    let detail = span_with_attrs(KindClass::Chat, Some(attrs));
    let mut term = Terminal::new(TestBackend::new(120, 10)).unwrap();
    let mut state = ChatDetailState::default();
    // Force-expand the diff path so the SystemDiff segments render.
    state.expanded.insert(NodeId::from("root/system"));
    term.draw(|f| {
        render(
            Rect::new(0, 0, 120, 10),
            f.buffer_mut(),
            &mut state,
            Some(("t", "s")),
            None,
            None,
            Some(&detail),
            Some(&prior_attrs),
            true,
        );
    })
    .unwrap();
    let s = buf_to_string(term.backend().buffer());
    assert!(s.contains(" ch · -") || s.contains("ch ·"), "missing diff meta in:\n{s}");
    // At least one REVERSED cell from the word-diff overlay.
    let buf = term.backend().buffer();
    let mut any_reversed = false;
    for y in 0..buf.area.height {
        for x in 0..buf.area.width {
            if buf[(x, y)].modifier.contains(ratatui::style::Modifier::REVERSED) {
                any_reversed = true;
            }
        }
    }
    assert!(any_reversed, "expected at least one REVERSED cell for diff segments");
}

#[test]
fn search_query_auto_expands_to_match() {
    let attrs = json!({
        "gen_ai.input.messages": [
            {"role":"user","parts":[{"type":"text","content":"needle in here"}]}
        ]
    });
    let detail = span_with_attrs(KindClass::Chat, Some(attrs));
    let mut term = Terminal::new(TestBackend::new(120, 20)).unwrap();
    let mut state = ChatDetailState::default();
    term.draw(|f| {
        render(
            Rect::new(0, 0, 120, 20),
            f.buffer_mut(),
            &mut state,
            Some(("t", "s")),
            Some("needle"),
            None,
            Some(&detail),
            None,
            true,
        );
    })
    .unwrap();
    let s = buf_to_string(term.backend().buffer());
    // Search auto-expand should make the matching message row visible.
    assert!(s.contains("user") || s.contains("message"), "expected expanded message row:\n{s}");
    assert!(state.expanded.contains(&NodeId::from("root/input")));
}

#[test]
fn search_query_clear_restores_user_expansion() {
    let attrs = json!({
        "gen_ai.input.messages": [
            {"role":"user","parts":[{"type":"text","content":"needle"}]}
        ]
    });
    let detail = span_with_attrs(KindClass::Chat, Some(attrs));
    let mut term = Terminal::new(TestBackend::new(80, 10)).unwrap();
    let mut state = ChatDetailState::default();
    // 1) Render with no query — capture user-clean state.
    term.draw(|f| {
        render(
            Rect::new(0, 0, 80, 10),
            f.buffer_mut(),
            &mut state,
            Some(("t", "s")),
            None,
            None,
            Some(&detail),
            None,
            true,
        );
    })
    .unwrap();
    let before: std::collections::HashSet<_> = state.expanded.clone();

    // 2) Apply a query — search-expand fires.
    term.draw(|f| {
        render(
            Rect::new(0, 0, 80, 10),
            f.buffer_mut(),
            &mut state,
            Some(("t", "s")),
            Some("needle"),
            None,
            Some(&detail),
            None,
            true,
        );
    })
    .unwrap();
    assert!(state.expanded.len() > before.len());

    // 3) Clear the query — snapshot restored.
    term.draw(|f| {
        render(
            Rect::new(0, 0, 80, 10),
            f.buffer_mut(),
            &mut state,
            Some(("t", "s")),
            Some(""),
            None,
            Some(&detail),
            None,
            true,
        );
    })
    .unwrap();
    assert_eq!(state.expanded, before);
}

#[test]
fn decode_prim_id_round_trip() {
    let nid = NodeId::from("root/input/input_messages/2__p3");
    assert_eq!(decode_prim_id(&nid).unwrap().as_str(), "root/input/input_messages/2");
    assert_eq!(decode_prim_index(&nid), Some(3));
    let nid = NodeId::from("root/input");
    assert!(decode_prim_id(&nid).is_none());
}
