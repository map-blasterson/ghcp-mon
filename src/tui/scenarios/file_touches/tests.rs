//! Rendering-level tests for the File Touches scenario using `TestBackend`.
//! Pure-logic tests live in the per-module `#[cfg(test)]` blocks (tree, walk).

use super::tree::{Touch, TouchKind, TouchRef};
use super::*;
use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::Rect;
use ratatui::Terminal;

fn touch(path: &str, kind: TouchKind) -> Touch {
    Touch {
        path: path.into(),
        kind,
        span_ref: TouchRef {
            span_id: path.into(),
            trace_id: "t".into(),
            span_pk: 1,
        },
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

fn draw(state: &mut FileTouchesState, session: Option<&str>, loaded: bool, touches: &[Touch]) -> String {
    let mut term = Terminal::new(TestBackend::new(60, 12)).unwrap();
    term.draw(|f| {
        let area = Rect::new(0, 0, 60, 12);
        render(area, f.buffer_mut(), state, session, loaded, touches, true);
    })
    .unwrap();
    buf_to_string(term.backend().buffer())
}

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

#[test]
fn empty_state_no_session() {
    let mut state = FileTouchesState::default();
    let out = draw(&mut state, None, false, &[]);
    assert!(out.contains("pick a session"), "{out}");
}

#[test]
fn empty_state_loading() {
    let mut state = FileTouchesState::default();
    let out = draw(&mut state, Some("cid"), false, &[]);
    assert!(out.contains("loading…"), "{out}");
}

#[test]
fn empty_state_no_touches() {
    let mut state = FileTouchesState::default();
    let out = draw(&mut state, Some("cid"), true, &[]);
    assert!(out.contains("no file touches yet"), "{out}");
}

#[test]
fn renders_tree_with_counts_and_indented_children() {
    let mut state = FileTouchesState::default();
    let touches = vec![
        touch("src/main.rs", TouchKind::Write),
        touch("src/main.rs", TouchKind::Read),
    ];
    let out = draw(&mut state, Some("cid"), true, &touches);
    // Header counts.
    assert!(out.contains("1 R / 1 W"), "{out}");
    // Dir row with expand glyph (auto-opened).
    assert!(out.contains("▾"), "{out}");
    assert!(out.contains("src"), "{out}");
    // File row indented under src with per-file counts.
    assert!(out.contains("main.rs"), "{out}");
}

#[test]
fn new_dir_auto_opens_then_preserves_user_collapse() {
    let mut state = FileTouchesState::default();
    let touches = vec![touch("src/a.rs", TouchKind::Read)];
    draw(&mut state, Some("cid"), true, &touches);
    assert!(state.open_dirs.contains("src"));
    assert!(state.known_dirs.contains("src"));

    // User collapses src.
    state.open_dirs.remove("src");

    // A re-render with a NEW dir appearing must auto-open the new dir but keep
    // src collapsed (src is already known).
    let touches2 = vec![
        touch("src/a.rs", TouchKind::Read),
        touch("docs/readme.md", TouchKind::Write),
    ];
    draw(&mut state, Some("cid"), true, &touches2);
    assert!(!state.open_dirs.contains("src"), "user collapse preserved");
    assert!(state.open_dirs.contains("docs"), "new dir auto-opened");
}

#[test]
fn collapsed_dir_hides_children() {
    let mut state = FileTouchesState::default();
    let touches = vec![touch("src/secret.rs", TouchKind::Read)];
    draw(&mut state, Some("cid"), true, &touches);
    // Collapse src.
    state.open_dirs.remove("src");
    let out = draw(&mut state, Some("cid"), true, &touches);
    assert!(out.contains("src"), "{out}");
    assert!(!out.contains("secret.rs"), "collapsed children hidden: {out}");
    // Collapse glyph present.
    assert!(out.contains("▸"), "{out}");
}

#[test]
fn expand_all_and_collapse_all_keys() {
    let mut state = FileTouchesState::default();
    let touches = vec![touch("src/tui/x.rs", TouchKind::Write)];
    draw(&mut state, Some("cid"), true, &touches);
    // Start collapsed.
    state.open_dirs.clear();

    // `+` expands all dirs.
    assert!(handle_key(key(KeyCode::Char('+')), &mut state));
    assert!(state.open_dirs.contains("src"));
    assert!(state.open_dirs.contains("src/tui"));

    // `-` collapses all.
    assert!(handle_key(key(KeyCode::Char('-')), &mut state));
    assert!(state.open_dirs.is_empty());
}

#[test]
fn expand_collapse_keys_are_noops_without_dirs() {
    let mut state = FileTouchesState::default();
    // Only top-level files → no directories.
    let touches = vec![touch("top.rs", TouchKind::Read)];
    draw(&mut state, Some("cid"), true, &touches);
    assert!(state.last_dir_paths.is_empty());

    // Both consumed but have no effect.
    assert!(handle_key(key(KeyCode::Char('+')), &mut state));
    assert!(state.open_dirs.is_empty());
    assert!(handle_key(key(KeyCode::Char('-')), &mut state));
    assert!(state.open_dirs.is_empty());
}

#[test]
fn disabled_controls_render_when_no_dirs() {
    let mut state = FileTouchesState::default();
    let touches = vec![touch("top.rs", TouchKind::Read)];
    let out = draw(&mut state, Some("cid"), true, &touches);
    // Controls still drawn (DIM); the labels are present.
    assert!(out.contains("[+] [-]"), "{out}");
}

#[test]
fn space_toggles_focused_dir() {
    let mut state = FileTouchesState::default();
    let touches = vec![touch("src/a.rs", TouchKind::Read)];
    draw(&mut state, Some("cid"), true, &touches);
    // Row 0 is the `src` dir.
    state.focus_row = 0;
    assert!(state.open_dirs.contains("src"));
    assert!(handle_key(key(KeyCode::Char(' ')), &mut state));
    assert!(!state.open_dirs.contains("src"));
    assert!(handle_key(key(KeyCode::Char(' ')), &mut state));
    assert!(state.open_dirs.contains("src"));
}
