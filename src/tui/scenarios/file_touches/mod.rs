//! File Touches scenario — the terminal port of the web `FileTouchesScenario`.
//!
//! Aggregates every file-touching tool call in the selected session into a
//! collapsible filesystem tree annotated with read/write counts. The column
//! resolves its data in [`crate::tui::app::App::draw_file_touches`] (session
//! span tree → matching tool spans → [`tree::Touch`] list) and hands the touch
//! list to [`render`], which builds the counting tree, auto-opens
//! newly-discovered directories, and paints the header + scrollable tree.
//!
//! ## Layout (top → bottom)
//! row 0: header — session marker + total `R / W` counts + `[+]`/`[-]` bulk
//! controls (rendered DIM/disabled when no directories are present).
//! rows 1..: scrollable tree — 2 cells of indent per level, a collapse glyph
//! (`▾`/`▸`) for directories and a blank for files, the name, then
//! right-aligned per-node `R / W` counts. The focused row is highlighted with
//! a cyan background.
//!
//! ## Key dispatch (column layer)
//! `↑`/`↓` move the row cursor; `←`/`→` collapse/expand the focused directory;
//! `Space` toggles it; `+`/`-` expand-all / collapse-all (no-ops when no
//! directories); `Home`/`End` jump to top/bottom.
//!
//! ## Live invalidation
//! No explicit live-feed subscription is needed: Phase 0's cache invalidation
//! table already maps `(span, span)` and `(derived, tool_call)` envelopes onto
//! the `["session-span-tree", *]` and `["span", ...]` keys this column reads,
//! and the draw-once-per-frame loop re-reads the cache on every `WsTick`.
//!
//! Source for (shared `frontend/llr/`):
//! - `File touches aggregates view edit create`
//! - `File touches builds filesystem tree with counts`
//! - `File touches new directories open by default`
//! - `File touches sort directories first then alphabetical`
//! - `File touches expand and collapse all controls`
//! - `File touches live invalidation on tool events`
//!
//! and (new `frontend/tui/llr/`):
//! - `TUI File touches tree row layout in cells`
//! - `TUI File touches header and bulk controls`
//! - `TUI File touches empty states`
//! - `TUI File touches preserves user collapse state across live updates`

pub mod tree;
pub mod walk;

use std::collections::HashSet;

use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Widget};

use tree::{build_tree, dir_paths, Touch, TouchNode, TouchTree};

/// Verbatim empty state shown when the column has no session configured.
pub const NO_SESSION_LINE: &str = "pick a session";
/// Verbatim empty state shown while the session span tree is still loading.
pub const LOADING_LINE: &str = "loading…";
/// Verbatim empty state shown once the tree is loaded but no file-touching
/// tool spans exist yet.
pub const NO_TOUCHES_LINE: &str = "no file touches yet";

/// Per-column File Touches scenario state. Owned by [`crate::tui::app::App`].
#[derive(Debug, Clone, Default)]
pub struct FileTouchesState {
    /// Directories currently expanded (keyed by full path).
    pub open_dirs: HashSet<String>,
    /// Directories ever seen — drives the "new directory auto-opens" rule so a
    /// dir is only force-opened the first time it appears.
    pub known_dirs: HashSet<String>,
    /// Row cursor within the rendered tree (0-based among visible rows).
    pub focus_row: usize,
    /// Vertical scroll offset (top visible tree row).
    pub scroll_top: u16,
    /// Visible row count from the last render (for cursor clamping).
    pub last_row_count: usize,
    /// Every directory path present in the last render (for expand-all and the
    /// `has_dirs` predicate used by `+`/`-`).
    pub last_dir_paths: Vec<String>,
    /// Row → directory path for the last render. `Some(path)` when the row is a
    /// directory (toggleable by `←`/`→`/`Space`), `None` for file rows.
    pub last_focus_dir: Vec<Option<String>>,
}

// ---------------------------------------------------------------------------
// Render
// ---------------------------------------------------------------------------

/// Render the File Touches column body.
///
/// * `session` — the configured conversation id (`None` → empty "pick a
///   session" state).
/// * `cache_loaded` — whether the `["session-span-tree", session]` cache entry
///   has a value yet (distinguishes "loading" from "no touches").
/// * `touches` — the file touches extracted from the session span tree (caller
///   resolves via [`walk::extract_touches`]).
pub fn render(
    area: Rect,
    buf: &mut Buffer,
    state: &mut FileTouchesState,
    session: Option<&str>,
    cache_loaded: bool,
    touches: &[Touch],
    _focused: bool,
) {
    if area.width == 0 || area.height == 0 {
        return;
    }

    // Empty states, in precedence order.
    if session.is_none() {
        paint_empty(area, buf, NO_SESSION_LINE);
        return;
    }
    if !cache_loaded {
        paint_empty(area, buf, LOADING_LINE);
        return;
    }
    if touches.is_empty() {
        paint_empty(area, buf, NO_TOUCHES_LINE);
        return;
    }

    let tree = build_tree(touches);
    let all_dirs = dir_paths(&tree);

    // New-directory auto-open: a dir only ever force-opens the first time it
    // appears; previously-seen dirs keep their current open/closed state.
    for d in &all_dirs {
        if !state.known_dirs.contains(d) {
            state.known_dirs.insert(d.clone());
            state.open_dirs.insert(d.clone());
        }
    }
    state.last_dir_paths = all_dirs.clone();

    // Header (row 0).
    let header = Rect::new(area.x, area.y, area.width, 1);
    render_header(header, buf, &tree, !all_dirs.is_empty());

    if area.height < 2 {
        return;
    }
    let tree_area = Rect::new(area.x, area.y + 1, area.width, area.height - 1);

    // Flatten the tree into visible rows honoring the open-dir set.
    let rows = build_rows(&tree, &state.open_dirs);
    state.last_row_count = rows.len();
    state.last_focus_dir = rows
        .iter()
        .map(|r| if r.is_dir { Some(r.path.clone()) } else { None })
        .collect();

    if state.focus_row >= rows.len() && !rows.is_empty() {
        state.focus_row = rows.len() - 1;
    }

    // Scroll-into-view for the focus row.
    let view_h = tree_area.height;
    if !rows.is_empty() && view_h > 0 {
        let fr = state.focus_row as u16;
        if fr < state.scroll_top {
            state.scroll_top = fr;
        } else if fr >= state.scroll_top + view_h {
            state.scroll_top = fr.saturating_sub(view_h.saturating_sub(1));
        }
        let max_scroll = (rows.len() as u16).saturating_sub(view_h);
        if state.scroll_top > max_scroll {
            state.scroll_top = max_scroll;
        }
    }

    let start = state.scroll_top as usize;
    for (vis, row) in rows.iter().skip(start).take(view_h as usize).enumerate() {
        let y = tree_area.y + vis as u16;
        let abs_idx = start + vis;
        let line = render_row(row, abs_idx == state.focus_row, tree_area.width);
        buf.set_line(tree_area.x, y, &line, tree_area.width);
    }
}

fn paint_empty(area: Rect, buf: &mut Buffer, text: &str) {
    let lines = vec![Line::from(Span::styled(
        text.to_string(),
        Style::default()
            .fg(Color::DarkGray)
            .add_modifier(Modifier::ITALIC),
    ))];
    Paragraph::new(lines).render(area, buf);
}

// ---------------------------------------------------------------------------
// Header
// ---------------------------------------------------------------------------

fn render_header(area: Rect, buf: &mut Buffer, tree: &TouchTree, has_dirs: bool) {
    // Top-level nodes already aggregate their subtree counts, so the totals are
    // the sum across the forest roots.
    let total_r: u64 = tree.root.iter().map(|n| n.reads).sum();
    let total_w: u64 = tree.root.iter().map(|n| n.writes).sum();

    let mut spans: Vec<Span<'static>> = vec![
        Span::styled(
            "⊞ files  ",
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!("({total_r} R / {total_w} W)"),
            Style::default().fg(Color::Gray),
        ),
    ];

    // Compute a gap that right-aligns the bulk controls when there's room.
    let left_w: usize = spans.iter().map(|s| s.content.chars().count()).sum();
    let ctrl = "[+] [-]";
    let ctrl_w = ctrl.chars().count();
    let total = area.width as usize;
    if total > left_w + ctrl_w + 1 {
        spans.push(Span::raw(" ".repeat(total - left_w - ctrl_w)));
    } else {
        spans.push(Span::raw(" "));
    }

    let ctrl_style = if has_dirs {
        Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
    } else {
        // Disabled controls render DIM (no directories to expand/collapse).
        Style::default()
            .fg(Color::DarkGray)
            .add_modifier(Modifier::DIM)
    };
    spans.push(Span::styled(ctrl.to_string(), ctrl_style));

    let line = Line::from(spans);
    buf.set_line(area.x, area.y, &line, area.width);
}

// ---------------------------------------------------------------------------
// Row builder + rendering
// ---------------------------------------------------------------------------

/// One flattened, visible tree row.
#[derive(Debug, Clone)]
struct Row {
    depth: u16,
    is_dir: bool,
    open: bool,
    name: String,
    path: String,
    reads: u64,
    writes: u64,
}

fn build_rows(tree: &TouchTree, open_dirs: &HashSet<String>) -> Vec<Row> {
    let mut out = Vec::new();
    push_level(&tree.root, 0, open_dirs, &mut out);
    out
}

fn push_level(nodes: &[TouchNode], depth: u16, open_dirs: &HashSet<String>, out: &mut Vec<Row>) {
    for n in nodes {
        let is_dir = n.is_dir();
        let open = is_dir && open_dirs.contains(&n.path);
        out.push(Row {
            depth,
            is_dir,
            open,
            name: n.name.clone(),
            path: n.path.clone(),
            reads: n.reads,
            writes: n.writes,
        });
        if is_dir && open {
            push_level(&n.children, depth + 1, open_dirs, out);
        }
    }
}

fn render_row(row: &Row, focused: bool, width: u16) -> Line<'static> {
    let glyph = if row.is_dir {
        if row.open {
            "▾"
        } else {
            "▸"
        }
    } else {
        " "
    };
    let indent = "  ".repeat(row.depth as usize);
    let left = format!("{indent}{glyph} {}", row.name);
    let counts = format!("{}R / {}W", row.reads, row.writes);

    let total = width as usize;
    let counts_w = counts.chars().count();
    // Reserve a 1-cell gap before the counts column.
    let avail_left = total.saturating_sub(counts_w + 1);
    let left = truncate(&left, avail_left);
    let left_w = left.chars().count();
    let pad = total.saturating_sub(left_w + counts_w);

    // A single full-width string lets the focus background cover the whole row.
    let text = format!("{left}{}{counts}", " ".repeat(pad));

    let style = if focused {
        Style::default()
            .bg(Color::Cyan)
            .fg(Color::Black)
            .add_modifier(Modifier::BOLD)
    } else if row.is_dir {
        Style::default()
            .fg(Color::Rgb(0x60, 0xa5, 0xfa))
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::White)
    };
    Line::from(Span::styled(text, style))
}

fn truncate(s: &str, max: usize) -> String {
    let count = s.chars().count();
    if count <= max {
        return s.to_string();
    }
    if max == 0 {
        return String::new();
    }
    let mut t: String = s.chars().take(max.saturating_sub(1)).collect();
    t.push('…');
    t
}

// ---------------------------------------------------------------------------
// Key dispatch
// ---------------------------------------------------------------------------

/// Handle a column-layer key. Returns `true` if consumed.
pub fn handle_key(key: KeyEvent, state: &mut FileTouchesState) -> bool {
    let has_dirs = !state.last_dir_paths.is_empty();
    match (key.code, key.modifiers) {
        (KeyCode::Up, _) => {
            state.focus_row = state.focus_row.saturating_sub(1);
            true
        }
        (KeyCode::Down, _) => {
            if state.last_row_count > 0 && state.focus_row + 1 < state.last_row_count {
                state.focus_row += 1;
            }
            true
        }
        (KeyCode::Home, _) => {
            state.focus_row = 0;
            true
        }
        (KeyCode::End, _) => {
            if state.last_row_count > 0 {
                state.focus_row = state.last_row_count - 1;
            }
            true
        }
        (KeyCode::Left, _) => {
            if let Some(Some(path)) = state.last_focus_dir.get(state.focus_row).cloned() {
                state.open_dirs.remove(&path);
            }
            true
        }
        (KeyCode::Right, _) => {
            if let Some(Some(path)) = state.last_focus_dir.get(state.focus_row).cloned() {
                state.open_dirs.insert(path);
            }
            true
        }
        (KeyCode::Char(' '), m) if !m.contains(KeyModifiers::CONTROL) => {
            if let Some(Some(path)) = state.last_focus_dir.get(state.focus_row).cloned() {
                if state.open_dirs.contains(&path) {
                    state.open_dirs.remove(&path);
                } else {
                    state.open_dirs.insert(path);
                }
            }
            true
        }
        (KeyCode::Char('+'), _) => {
            // Expand all (no-op when no directories present).
            if has_dirs {
                state.open_dirs = state.last_dir_paths.iter().cloned().collect();
            }
            true
        }
        (KeyCode::Char('-'), _) => {
            // Collapse all (no-op when no directories present).
            if has_dirs {
                state.open_dirs.clear();
            }
            true
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests;
