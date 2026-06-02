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
    // definitions get the `Unchanged` badge at the top-level frontier and
    // should paint DARK (`▓`). Input/output messages don't carry an
    // Unchanged badge at the section level (input falls through to per-turn
    // delta; output isn't diffed) so they remain FULL (`█`). We assert the
    // mixed outcome.
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
    let mut dark = 0;
    let mut full = 0;
    for x in 0..80 {
        match buf[(x, 1)].symbol() {
            "▓" => dark += 1,
            "█" => full += 1,
            _ => {}
        }
    }
    assert!(dark > 0, "expected DARK (▓) cells from sys+tools, got dark={dark} full={full}");
    assert!(full > 0, "expected FULL (█) cells from input+output, got dark={dark} full={full}");
}

#[test]
fn summary_bar_uses_full_glyph_in_full_mode_even_with_prior() {
    // Same setup as the dark-shade test, but FULL mode never tags Unchanged
    // → bar should be all FULL (`█`), no DARK.
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
    let mut dark = 0;
    let mut full = 0;
    for x in 0..80 {
        match buf[(x, 1)].symbol() {
            "▓" => dark += 1,
            "█" => full += 1,
            _ => {}
        }
    }
    assert_eq!(dark, 0, "FULL mode must not shade segments");
    assert!(full > 0, "expected FULL (█) cells in FULL mode, got full={full}");
}

#[test]
fn tool_call_arrow_appears_at_target_message_row() {
    let attrs = json!({
        "gen_ai.input.messages": [
            {"role":"tool","parts":[
                {"type":"tool_call_response","id":"call_X","result":"ok"}
            ]}
        ]
    });
    let detail = span_with_attrs(KindClass::Chat, Some(attrs));
    let mut term = Terminal::new(TestBackend::new(100, 15)).unwrap();
    let mut state = ChatDetailState::default();
    // Pre-expand root + input + input_messages so the message row is in view.
    state.expanded.insert(NodeId::from("root"));
    state.expanded.insert(NodeId::from("root/input"));
    term.draw(|f| {
        render(
            Rect::new(0, 0, 100, 15),
            f.buffer_mut(),
            &mut state,
            Some(("t", "s")),
            None,
            Some("call_X"),
            Some(&detail),
            None,
            true,
        );
    })
    .unwrap();
    let s = buf_to_string(term.backend().buffer());
    assert!(s.contains('▶'), "expected ▶ arrow in body:\n{s}");
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
