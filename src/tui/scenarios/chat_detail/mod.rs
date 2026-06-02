//! Chat Detail scenario — the GenAI content-attribute breakdown column.
//!
//! Layout (top → bottom inside the column body):
//!  * row 0: header strip — `chat   bytes {N}   mode: [DELTA|FULL]   m`
//!  * row 1: summary bar — one colored segment per visible-frontier node,
//!    widths proportional to `bytes` per the `Chat detail summary bar
//!    proportional to visible segments` LLR.
//!  * row 2: hover indicator — yellow `▲` characters spanning the cell range
//!    of the segment (or ancestor's contiguous descendants) matching the
//!    focused tree row; blank when no segment matches.
//!  * rows 3..: tree with two prefix cells — arrow gutter (1 cell, `▶` only
//!    at the tool-call target row) + focus glyph (1 cell, `▸`/`▾`/space).
//!
//! Source for (shared `frontend/llr/`):
//! - `Chat detail only renders for chat span`
//! - `Chat detail tree built from four content attributes`
//! - `Chat detail bytes computed via JSON length`
//! - `Chat detail summary bar proportional to visible segments`
//! - `Chat detail mode toggle DELTA FULL`
//! - `Chat detail DELTA diffs against prior chat span`
//! - `Chat detail tool-call hint auto-expand and arrow`
//! - `Chat detail key cursor icon follows pointer`
//! - `Chat detail long primitives click to expand`
//! - `ChatDetail auto-expands tree to span search matches`
//! - `Detail columns pass span search query to TextBlocks`
//! - `Message view renders parts by type`
//!
//! and (new `frontend/tui/llr/`):
//! - `TUI Chat detail layout in cells`
//! - `TUI Chat detail tool-call arrow gutter`
//! - `TUI Chat detail key-cursor indicator on focused key row`
//! - `TUI Chat detail node id is slash-delimited path`
//! - `TUI Chat detail focus precedence within column`
//! - `TUI Chat detail mode chip in header`
//! - `TUI Chat detail search-expanded set tracks restoration`
//! - `TUI Chat detail summary bar paints via Buffer cell_mut`

pub mod diff_segments;
pub mod messages;
pub mod search_expand;
pub mod tool_call_hint;
pub mod tree;
pub mod walk;

use std::collections::{HashMap, HashSet};

use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Widget};
use serde_json::Value;

use crate::tui::format::{fmt_compact_count, pretty_json};
use crate::tui::model::{KindClass, SpanDetail};
use crate::tui::scenarios::chat_detail::diff_segments::DiffSegment;
use crate::tui::scenarios::chat_detail::messages::{has_captured_content, Part};
use crate::tui::scenarios::chat_detail::tree::{
    build_tree, ChatContent, ChatMode, NodeBadge, NodeId, NodeKind, TreeNode,
};
use crate::tui::widgets::searchable_text_block::SearchableTextBlockState;
use crate::tui::widgets::summary_bar::{SummaryBar, SummarySeg};

/// Verbatim empty state when the selected span is not chat-kind.
pub const NOT_A_CHAT_LINE: &str = "selected span is not a chat span";

/// Verbatim empty state when no chat span is selected at all.
pub const NO_SELECTION_LINE: &str = "Select a chat span in the Spans column.";

/// Verbatim empty state when no content was captured (env-var hint).
pub const NO_CONTENT_LINE: &str = "no content captured — set OTEL_INSTRUMENTATION_GENAI_CAPTURE_MESSAGE_CONTENT=true and OTEL_SEMCONV_STABILITY_OPT_IN=gen_ai_latest_experimental";

/// Threshold (in chars of stringified value) above which a primitive is
/// rendered through a collapsible block per the LLR.
pub const LONG_PRIM_THRESHOLD: usize = 200;

/// Per-column chat-detail scenario state. Owned by [`crate::tui::app::App`].
#[derive(Debug, Clone, Default)]
pub struct ChatDetailState {
    /// DELTA / FULL mode chip (toggled by `m`). Seeded from the column
    /// config's `chat_mode` on first access; subsequent toggles do not
    /// write back to config — that's existing behavior, not changed here.
    pub mode: ChatMode,
    /// User+search expansion union actually rendered.
    pub expanded: HashSet<NodeId>,
    /// Toggled by `Space` on a primitive key row; keyed by (node, primitive
    /// index).
    pub expanded_prims: HashSet<(NodeId, usize)>,
    /// User-driven expansions (excludes search/auto-expand additions); used
    /// to restore state when `search_query` clears.
    pub user_expanded: HashSet<NodeId>,
    /// When non-empty, holds the `expanded` snapshot taken at the moment the
    /// active search began. Restored when search clears.
    pub search_expanded_snapshot: Option<HashSet<NodeId>>,
    /// Last column-level search query observed; drives snapshot lifecycle.
    pub last_search_query: String,
    /// Row cursor within the rendered body (0-based among visible rows).
    pub focus_row: usize,
    /// Vertical scroll offset.
    pub scroll_top: u16,
    /// Per-searchable-block state, keyed by a stable block key (the node id
    /// for part bodies, `"{node_id}|prim/{i}"` for primitives).
    pub text_blocks: HashMap<String, SearchableTextBlockState>,
    /// Row count of the last rendered body — used for scroll clamping.
    pub last_body_len: u16,
    /// View height of the last render.
    pub last_view_h: u16,
    /// Total row count of the rendered tree from the last render.
    pub last_row_count: usize,
    /// Captured row → NodeId map from the last render, used by key dispatch
    /// to translate `focus_row` into the node ID the user is pointing at.
    /// Entries for primitive synthetic ids (`{node}__p{i}`) are present too;
    /// `handle_key` decodes them.
    pub last_focus_map: Vec<Option<NodeId>>,
}

impl ChatDetailState {
    #[allow(dead_code)]
    fn max_scroll(&self) -> u16 {
        self.last_body_len.saturating_sub(self.last_view_h)
    }
}

// ---------------------------------------------------------------------------
// Public render entry point
// ---------------------------------------------------------------------------

/// Render the chat-detail column body. Reads:
///  * `selection` — `(trace_id, span_id)` from the column config.
///  * `search_query` — column-level external search (yellow overlay + auto-
///    expand source).
///  * `selected_tool_call_id` — drives the arrow gutter + non-destructive
///    auto-expand.
///  * `mode` — DELTA / FULL.
///  * `detail` — the cached `SpanDetail` for the selection (caller resolves).
///  * `prior_chat_attrs` — the captured attrs of the prior chat span (DELTA
///    baseline), computed by the caller from the complete cached
///    `session-span-tree`.
#[allow(clippy::too_many_arguments)]
pub fn render(
    area: Rect,
    buf: &mut Buffer,
    state: &mut ChatDetailState,
    selection: Option<(&str, &str)>,
    search_query: Option<&str>,
    selected_tool_call_id: Option<&str>,
    detail: Option<&SpanDetail>,
    prior_chat_attrs: Option<&Value>,
    _focused: bool,
) {
    if area.width == 0 || area.height == 0 {
        return;
    }

    // Empty states first.
    let lines = match (selection, detail) {
        (None, _) => empty_lines(NO_SELECTION_LINE),
        (Some(_), None) => empty_lines("loading…"),
        (Some(_), Some(d)) => {
            if d.span.kind_class != KindClass::Chat {
                empty_lines(NOT_A_CHAT_LINE)
            } else {
                return render_full(
                    area,
                    buf,
                    state,
                    search_query,
                    selected_tool_call_id,
                    d,
                    prior_chat_attrs,
                );
            }
        }
    };

    state.last_body_len = lines.len() as u16;
    state.last_view_h = area.height;
    Paragraph::new(lines).render(area, buf);
}

fn empty_lines(text: &str) -> Vec<Line<'static>> {
    vec![Line::from(Span::styled(
        text.to_string(),
        Style::default().fg(Color::DarkGray).add_modifier(Modifier::ITALIC),
    ))]
}

/// Full render path for a chat-kind span.
fn render_full(
    area: Rect,
    buf: &mut Buffer,
    state: &mut ChatDetailState,
    search_query: Option<&str>,
    selected_tool_call_id: Option<&str>,
    detail: &SpanDetail,
    prior_chat_attrs: Option<&Value>,
) {
    let mode = state.mode;
    let cur_attrs = detail.span.attributes.clone().unwrap_or(Value::Null);
    let current = ChatContent::from_attrs(&cur_attrs);

    // No content captured guard (only when truly empty).
    if !has_captured_content(&cur_attrs) {
        let lines = vec![Line::from(Span::styled(
            NO_CONTENT_LINE.to_string(),
            Style::default().fg(Color::DarkGray).add_modifier(Modifier::ITALIC),
        ))];
        state.last_body_len = lines.len() as u16;
        state.last_view_h = area.height;
        Paragraph::new(lines).render(area, buf);
        return;
    }

    // DELTA baseline.
    let prior = prior_chat_attrs.map(|a| {
        let pc = ChatContent::from_attrs(a);
        // Per LLR: when prior captured content is empty, degrade to FULL.
        (pc, has_captured_content(a))
    });
    let prior_for_build = match (mode, &prior) {
        (ChatMode::Delta, Some((pc, true))) => Some(pc),
        _ => None,
    };
    let tree = build_tree(&current, prior_for_build, mode);

    // Reconcile search-expansion lifecycle.
    let q = search_query.unwrap_or("");
    let q_changed = q != state.last_search_query;
    if q_changed {
        match (state.last_search_query.is_empty(), q.is_empty()) {
            (true, false) => {
                // Snapshot user state and apply search-expand.
                state.search_expanded_snapshot = Some(state.expanded.clone());
            }
            (false, true) => {
                // Restore.
                if let Some(snap) = state.search_expanded_snapshot.take() {
                    state.expanded = snap;
                }
            }
            _ => {}
        }
        state.last_search_query = q.to_string();
    }
    if !q.is_empty() {
        let add = search_expand::search_expanded(&tree, q, mode);
        for id in add {
            state.expanded.insert(id);
        }
    }

    // Tool-call hint: non-destructive auto-expand.
    let (arrow_target, _tc_added) = if let Some(tcid) = selected_tool_call_id {
        if let Some((set, target)) = tool_call_hint::auto_expand_for_tool_call(&tree, tcid) {
            for id in &set {
                state.expanded.insert(id.clone());
            }
            (Some(target), true)
        } else {
            (None, false)
        }
    } else {
        (None, false)
    };

    // ---- Layout ----
    if area.height < 4 {
        Paragraph::new(empty_lines("(column too short)")).render(area, buf);
        return;
    }
    let header = Rect::new(area.x, area.y, area.width, 1);
    let bar = Rect::new(area.x, area.y + 1, area.width, 1);
    let indicator = Rect::new(area.x, area.y + 2, area.width, 1);
    let tree_area = Rect::new(area.x, area.y + 3, area.width, area.height - 3);

    // ---- Header ----
    render_header(header, buf, tree.bytes, mode);

    // ---- Summary bar segments ----
    let frontier = walk::visible_frontier(&tree, &state.expanded);
    let segs: Vec<SummarySeg> = frontier
        .iter()
        .map(|n| SummarySeg {
            id: n.id.0.clone(),
            bytes: n.bytes,
            color: color_for_node(n),
            label: None,
        })
        .collect();

    // Build flat row list, then clamp focus_row BEFORE deriving the hovered
    // id so the indicator never points off the end of the visible list.
    let rows = build_rows(&tree, state, arrow_target.as_ref());
    state.last_row_count = rows.len();
    state.last_focus_map = rows.iter().map(|r| r.node_id.clone()).collect();
    if state.focus_row >= rows.len() && !rows.is_empty() {
        state.focus_row = rows.len() - 1;
    }
    let hovered_id = rows
        .get(state.focus_row)
        .and_then(|r| r.node_id.as_ref())
        .map(|n| n.0.as_str());

    // ---- Summary bar + hover indicator ----
    let summary = SummaryBar {
        segments: &segs,
        hovered: hovered_id,
    };
    summary.render(bar, buf);
    summary.render_hover_indicator(indicator, buf);

    // ---- Tree body ----
    // Scroll-into-view for focus row.
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
    state.last_body_len = rows.len() as u16;
    state.last_view_h = view_h;

    let start = state.scroll_top as usize;
    for (vis, row) in rows.iter().skip(start).take(view_h as usize).enumerate() {
        let y = tree_area.y + vis as u16;
        let abs_idx = start + vis;
        let line = render_row(row, abs_idx == state.focus_row, q);
        buf.set_line(tree_area.x, y, &line, tree_area.width);
    }
}

// ---------------------------------------------------------------------------
// Header
// ---------------------------------------------------------------------------

fn render_header(area: Rect, buf: &mut Buffer, total_bytes: usize, mode: ChatMode) {
    let bytes_label = fmt_compact_count(total_bytes as i64);
    let mut spans: Vec<Span<'static>> = vec![
        Span::styled(
            "⊞ chat  ",
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!("bytes {bytes_label}  "),
            Style::default().fg(Color::Gray),
        ),
        Span::styled("mode: ", Style::default().fg(Color::Gray)),
        Span::styled(
            format!("[{}]", mode.label()),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled("  m=toggle", Style::default().fg(Color::DarkGray)),
    ];
    let line = Line::from(std::mem::take(&mut spans));
    buf.set_line(area.x, area.y, &line, area.width);
}

// ---------------------------------------------------------------------------
// Row builder
// ---------------------------------------------------------------------------

/// One rendered row inside the tree area.
#[derive(Debug, Clone)]
struct RenderRow {
    indent: u16,
    /// Tree glyph: ▸/▾/blank.
    glyph: &'static str,
    /// True iff this row is the arrow-target message (`▶` in gutter).
    arrow: bool,
    /// Right-aligned meta text.
    meta: Option<String>,
    /// Optional left badge (REMOVED/ADDED/UNCHANGED/CHANGED chip).
    badge: Option<NodeBadge>,
    /// Main row label (already styled-text-free; styling happens at render).
    label: String,
    /// Node ID this row represents (None for non-node rows like primitive
    /// values).
    node_id: Option<NodeId>,
    /// `(+)` / `(-)` indicator for a focused long-primitive key row.
    expand_indicator: Option<&'static str>,
    /// Style override for the label (used to dim diff-removed segments and
    /// brighten diff-added ones in inline content rows).
    label_styles: Option<Vec<(String, Style)>>,
}

impl RenderRow {
    fn plain(label: impl Into<String>, indent: u16, node_id: Option<NodeId>) -> Self {
        Self {
            indent,
            glyph: "  ",
            arrow: false,
            meta: None,
            badge: None,
            label: label.into(),
            node_id,
            expand_indicator: None,
            label_styles: None,
        }
    }
}

fn build_rows(
    root: &TreeNode,
    state: &ChatDetailState,
    arrow_target: Option<&NodeId>,
) -> Vec<RenderRow> {
    let mut out: Vec<RenderRow> = Vec::new();
    for child in &root.children {
        push_node(child, 0, state, arrow_target, &mut out);
    }
    out
}

fn push_node(
    node: &TreeNode,
    indent: u16,
    state: &ChatDetailState,
    arrow_target: Option<&NodeId>,
    out: &mut Vec<RenderRow>,
) {
    let has_children = !node.children.is_empty() || !node.primitives.is_empty();
    let expanded = state.expanded.contains(&node.id);
    let glyph = if !has_children {
        "  "
    } else if expanded {
        "▾ "
    } else {
        "▸ "
    };
    let arrow = arrow_target.map(|t| t == &node.id).unwrap_or(false);

    let mut label = node.label.clone();
    // Append bytes inline as a dim suffix.
    if node.bytes > 0 {
        label = format!("{label}  ({} b)", fmt_compact_count(node.bytes as i64));
    }
    let row = RenderRow {
        indent,
        glyph,
        arrow,
        meta: node.meta.clone(),
        badge: node.badge,
        label,
        node_id: Some(node.id.clone()),
        expand_indicator: None,
        label_styles: None,
    };
    out.push(row);

    // SystemDiff is a leaf-style node whose value IS the segment row; always
    // emit it inline (no expansion gate). Avoids the "diff invisible until
    // user expands the SystemDiff node" trap.
    if let NodeKind::SystemDiff(segs) = &node.kind {
        let mut buf = String::new();
        let mut styled: Vec<(String, Style)> = Vec::new();
        for seg in segs {
            let (t, st) = match seg {
                DiffSegment::Unchanged(t) => (t.clone(), Style::default().fg(Color::Gray)),
                DiffSegment::Added(t) => (
                    t.clone(),
                    Style::default()
                        .fg(Color::Black)
                        .bg(Color::Rgb(0x4a, 0xde, 0x80))
                        .add_modifier(Modifier::REVERSED | Modifier::BOLD),
                ),
                DiffSegment::Removed(t) => (
                    t.clone(),
                    Style::default()
                        .fg(Color::Black)
                        .bg(Color::Rgb(0xff, 0x80, 0x80))
                        .add_modifier(Modifier::REVERSED | Modifier::DIM),
                ),
            };
            styled.push((t.clone(), st));
            buf.push_str(&t);
        }
        let mut row = RenderRow::plain(buf, indent + 1, None);
        row.label_styles = Some(styled);
        out.push(row);
    }

    if !expanded {
        return;
    }

    // Primitives (key=value rows).
    for (i, (k, v)) in node.primitives.iter().enumerate() {
        let pkey = (node.id.clone(), i);
        let value_str = match v {
            Value::String(s) => s.clone(),
            other => pretty_json(other),
        };
        let is_long = value_str.len() > LONG_PRIM_THRESHOLD || value_str.contains('\n');
        let open = state.expanded_prims.contains(&pkey);
        let display_value = if is_long && !open {
            let mut t: String = value_str.chars().take(LONG_PRIM_THRESHOLD).collect();
            t.push_str(" …");
            t.replace('\n', " ⏎ ")
        } else if !is_long {
            value_str.clone()
        } else {
            value_str.clone()
        };
        let indicator = if is_long {
            if open { Some("(-)") } else { Some("(+)") }
        } else {
            None
        };
        // Key row.
        let mut key_row = RenderRow::plain(
            format!("{k}: {display_value}"),
            indent + 1,
            // Use a synthetic NodeId so focus tracking maps back to the
            // primitive key (re-using the child slot in `expanded_prims`).
            Some(NodeId(format!("{}__p{}", node.id.0, i))),
        );
        key_row.expand_indicator = indicator;
        out.push(key_row);
        // Expanded value: render wrapped (each line indented).
        if is_long && open {
            for line in value_str.lines() {
                out.push(RenderRow::plain(line.to_string(), indent + 2, None));
            }
        }
    }

    // Special-case rendering inside known kinds:
    match &node.kind {
        NodeKind::SystemDiff(_) => {
            // Already emitted above (leaf-inline).
        }
        NodeKind::Part(p) => {
            // For text/reasoning, render the content as additional rows.
            match p {
                Part::Text { content, .. } | Part::Reasoning { content, .. } => {
                    for line in content.lines().take(8) {
                        out.push(RenderRow::plain(line.to_string(), indent + 1, None));
                    }
                    if content.lines().count() > 8 {
                        out.push(RenderRow::plain("…".to_string(), indent + 1, None));
                    }
                }
                Part::ToolCall { name, id, arguments, .. } => {
                    out.push(RenderRow::plain(
                        format!("call: {name}  id={}", short_id(id)),
                        indent + 1,
                        None,
                    ));
                    let args = pretty_json(arguments);
                    for line in args.lines().take(12) {
                        out.push(RenderRow::plain(line.to_string(), indent + 2, None));
                    }
                    if args.lines().count() > 12 {
                        out.push(RenderRow::plain("…".to_string(), indent + 2, None));
                    }
                }
                Part::ToolCallResponse { id, result, .. } => {
                    out.push(RenderRow::plain(
                        format!("response: id={}", short_id(id)),
                        indent + 1,
                        None,
                    ));
                    let rendered = match result {
                        Value::String(s) => s.clone(),
                        other => pretty_json(other),
                    };
                    for line in rendered.lines().take(12) {
                        out.push(RenderRow::plain(line.to_string(), indent + 2, None));
                    }
                    if rendered.lines().count() > 12 {
                        out.push(RenderRow::plain("…".to_string(), indent + 2, None));
                    }
                }
                Part::Other { raw } => {
                    let rendered = pretty_json(raw);
                    for line in rendered.lines().take(8) {
                        out.push(RenderRow::plain(line.to_string(), indent + 1, None));
                    }
                }
            }
        }
        _ => {}
    }

    for c in &node.children {
        push_node(c, indent + 1, state, arrow_target, out);
    }
}

fn short_id(s: &str) -> String {
    s.chars().take(8).collect()
}

// ---------------------------------------------------------------------------
// Row rendering
// ---------------------------------------------------------------------------

fn render_row(row: &RenderRow, focused: bool, search_query: &str) -> Line<'static> {
    // Build: [arrow gutter 1c][focus glyph 1c (▶/space)][indent][glyph][badge?][label][meta?]
    let mut spans: Vec<Span<'static>> = Vec::new();
    // Arrow gutter.
    let arrow_span = if row.arrow {
        Span::styled(
            "▶",
            Style::default()
                .fg(Color::Rgb(0xfd, 0xe0, 0x47))
                .add_modifier(Modifier::BOLD),
        )
    } else {
        Span::raw(" ")
    };
    spans.push(arrow_span);
    // Focus marker.
    spans.push(if focused {
        Span::styled("▸", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
    } else {
        Span::raw(" ")
    });
    // Indent (2 cells per level).
    if row.indent > 0 {
        spans.push(Span::raw(" ".repeat(row.indent as usize * 2)));
    }
    // Glyph (collapse/expand).
    spans.push(Span::styled(row.glyph.to_string(), Style::default().fg(Color::DarkGray)));
    // Badge.
    if let Some(b) = row.badge {
        spans.push(badge_span(b));
        spans.push(Span::raw(" "));
    }
    // Label — with optional styled segments (used for SystemDiff).
    if let Some(styled) = &row.label_styles {
        for (t, st) in styled {
            let painted = paint_with_search(t, *st, search_query);
            spans.extend(painted);
        }
    } else {
        let base = if focused {
            Style::default().fg(Color::White).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::White)
        };
        spans.extend(paint_with_search(&row.label, base, search_query));
    }
    // Expand indicator.
    if let Some(ind) = row.expand_indicator {
        spans.push(Span::raw(" "));
        spans.push(Span::styled(
            ind.to_string(),
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
        ));
    }
    // Meta.
    if let Some(m) = &row.meta {
        spans.push(Span::raw("  "));
        spans.push(Span::styled(
            format!("[{m}]"),
            Style::default()
                .fg(Color::DarkGray)
                .add_modifier(Modifier::ITALIC),
        ));
    }
    Line::from(spans)
}

fn paint_with_search(text: &str, base: Style, query: &str) -> Vec<Span<'static>> {
    if query.is_empty() {
        return vec![Span::styled(text.to_string(), base)];
    }
    let needle = query.to_lowercase();
    let hay = text.to_lowercase();
    let mut out: Vec<Span<'static>> = Vec::new();
    let mut last = 0usize;
    let mut start = 0usize;
    while let Some(pos) = hay[start..].find(&needle) {
        let abs = start + pos;
        if abs > last {
            out.push(Span::styled(text[last..abs].to_string(), base));
        }
        let end = abs + needle.len();
        out.push(Span::styled(
            text[abs..end].to_string(),
            Style::default()
                .bg(Color::Yellow)
                .fg(Color::Black)
                .add_modifier(Modifier::BOLD),
        ));
        last = end;
        start = end;
    }
    if last < text.len() {
        out.push(Span::styled(text[last..].to_string(), base));
    }
    out
}

fn badge_span(b: NodeBadge) -> Span<'static> {
    let (label, color) = match b {
        NodeBadge::Unchanged => ("=", Color::DarkGray),
        NodeBadge::Changed => ("Δ", Color::Yellow),
        NodeBadge::Added => ("+", Color::Green),
        NodeBadge::Removed => ("-", Color::Red),
    };
    Span::styled(
        format!("[{label}]"),
        Style::default().fg(color).add_modifier(Modifier::BOLD),
    )
}

fn color_for_node(node: &TreeNode) -> Color {
    // Top-level branches get fixed colours; nested nodes inherit by depth.
    // Use the first segment after "root/" to choose.
    let after_root = node.id.0.strip_prefix("root/").unwrap_or(&node.id.0);
    let branch = after_root.split('/').next().unwrap_or("");
    match branch {
        "system" => Color::Rgb(0x60, 0xa5, 0xfa),  // blue
        "tools" => Color::Rgb(0xfd, 0xe0, 0x47),   // yellow
        "input" => Color::Rgb(0x4a, 0xde, 0x80),   // green
        "output" => Color::Rgb(0xfb, 0x92, 0x3c),  // orange
        _ => Color::Gray,
    }
}

// ---------------------------------------------------------------------------
// Key dispatch
// ---------------------------------------------------------------------------

/// Handle a column-layer key. Returns `true` if consumed.
pub fn handle_key(
    key: KeyEvent,
    state: &mut ChatDetailState,
) -> bool {
    use crate::tui::widgets::searchable_text_block::{SearchPhase, SearchableTextBlock};
    // `Enter` / `Shift+Enter` cycle search matches in the first Active
    // searchable text block (the Spans search box propagates an external
    // query which sets these blocks Active via the external_active latch).
    // Per Phase 6's `TUI Chat detail focus precedence within column` LLR
    // update, Tab no longer navigates within the column, so we don't have
    // a per-block focused state to consult — route to the first Active
    // block.
    if matches!(key.code, KeyCode::Enter) {
        let first_active = state
            .text_blocks
            .iter()
            .find_map(|(k, st)| (st.phase == SearchPhase::Active).then(|| k.clone()));
        if let Some(k) = first_active {
            if let Some(st) = state.text_blocks.get_mut(&k) {
                if SearchableTextBlock::handle_key(key, st, None) {
                    return true;
                }
            }
        }
    }

    match (key.code, key.modifiers) {
        (KeyCode::Tab | KeyCode::BackTab, _) => false,
        (KeyCode::Char('m'), m) if !m.contains(KeyModifiers::CONTROL) => {
            state.mode = state.mode.toggled();
            true
        }
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
            if let Some(Some(nid)) = state.last_focus_map.get(state.focus_row).cloned() {
                if let Some(real_id) = decode_prim_id(&nid) {
                    // On a primitive key row, Left collapses the parent.
                    state.user_expanded.remove(&real_id);
                    state.expanded.remove(&real_id);
                } else {
                    state.user_expanded.remove(&nid);
                    state.expanded.remove(&nid);
                }
            }
            true
        }
        (KeyCode::Right, _) => {
            if let Some(Some(nid)) = state.last_focus_map.get(state.focus_row).cloned() {
                if let Some(real_id) = decode_prim_id(&nid) {
                    // Right on a primitive key opens it (same as Space).
                    if let Some(i) = decode_prim_index(&nid) {
                        state.expanded_prims.insert((real_id, i));
                    }
                } else {
                    state.user_expanded.insert(nid.clone());
                    state.expanded.insert(nid);
                }
            }
            true
        }
        (KeyCode::Char(' '), m) if !m.contains(KeyModifiers::CONTROL) => {
            if let Some(Some(nid)) = state.last_focus_map.get(state.focus_row).cloned() {
                if let (Some(real_id), Some(i)) = (decode_prim_id(&nid), decode_prim_index(&nid)) {
                    // Toggle primitive expansion.
                    let key = (real_id, i);
                    if state.expanded_prims.contains(&key) {
                        state.expanded_prims.remove(&key);
                    } else {
                        state.expanded_prims.insert(key);
                    }
                } else {
                    // Toggle node expansion.
                    if state.user_expanded.contains(&nid) {
                        state.user_expanded.remove(&nid);
                        state.expanded.remove(&nid);
                    } else {
                        state.user_expanded.insert(nid.clone());
                        state.expanded.insert(nid);
                    }
                }
            }
            true
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests;

/// `Scenario` impl bundling per-column state + the existing pure handlers.
/// Owns `ChatDetailState` (mode + tree + searchable text blocks). Pulls the
/// DELTA prior-chat-span attrs through `Ctx` so the scenario stays
/// decoupled from `App`.
#[derive(Debug, Default)]
pub struct ChatDetailScenario {
    pub state: ChatDetailState,
    /// True once the per-column state's `mode` has been initialised from
    /// `config["chat_mode"]`. Persisted reload happens once per scenario
    /// instance; subsequent draws keep the live `state.mode` value (the
    /// `m` key toggle in `handle_key` writes to it).
    mode_seeded: bool,
}

impl ChatDetailScenario {
    pub fn new() -> Self {
        Self::default()
    }

    fn ensure_mode_seeded(&mut self, config: &crate::tui::workspace::ColumnConfig) {
        if self.mode_seeded {
            return;
        }
        let cfg_mode = config.get("chat_mode").and_then(|v| v.as_str());
        self.state.mode = tree::ChatMode::from_config_str(cfg_mode);
        self.mode_seeded = true;
    }
}

impl crate::tui::scenarios::Scenario for ChatDetailScenario {
    fn draw(
        &mut self,
        ctx: &mut crate::tui::scenarios::Ctx<'_>,
        _col_idx: usize,
        _col_id: &str,
        config: &crate::tui::workspace::ColumnConfig,
        area: Rect,
        buf: &mut Buffer,
        focused: bool,
        _outcome: &mut crate::tui::app::DrawOutcome,
    ) {
        self.ensure_mode_seeded(config);

        let trace_id = config.get("selected_trace_id").and_then(|v| v.as_str());
        let span_id = config.get("selected_span_id").and_then(|v| v.as_str());
        let selected_tool_call_id =
            config.get("selected_tool_call_id").and_then(|v| v.as_str());
        let search_query = config.get("search_query").and_then(|v| v.as_str());
        let selection = match (trace_id, span_id) {
            (Some(t), Some(s)) => Some((t, s)),
            _ => None,
        };
        let detail = selection.and_then(|(t, s)| ctx.cached_span_detail(t, s));

        // DELTA prior-chat-span lookup walks the COMPLETE cached
        // session-span-tree (per the cache contract — NOT the reveal-
        // filtered Spans view).
        let prior_attrs: Option<Value> = match (&detail, self.state.mode) {
            (Some(d), tree::ChatMode::Delta) => {
                use crate::tui::model::SpanTreeExt;
                d.projection
                    .chat_turn
                    .as_ref()
                    .and_then(|c| c.conversation_id.clone())
                    .and_then(|cid| {
                        let tree = ctx.cached_session_span_tree(&cid);
                        let prior = tree.find_prior_chat(
                            d.span.span_pk,
                            d.span.end_unix_ns,
                            d.span.start_unix_ns,
                        )?;
                        let prior_tid = prior.trace_id.clone();
                        let prior_sid = prior.span_id.clone();
                        ctx.cached_span_detail(&prior_tid, &prior_sid)
                            .and_then(|sd| sd.span.attributes.clone())
                    })
            }
            _ => None,
        };

        render(
            area,
            buf,
            &mut self.state,
            selection,
            search_query,
            selected_tool_call_id,
            detail.as_deref(),
            prior_attrs.as_ref(),
            focused,
        );
    }

    fn handle_key(
        &mut self,
        _ctx: &mut crate::tui::scenarios::Ctx<'_>,
        _col_idx: usize,
        _col_id: &str,
        config: &crate::tui::workspace::ColumnConfig,
        k: KeyEvent,
    ) -> crate::tui::scenarios::KeyOutcome {
        self.ensure_mode_seeded(config);
        let consumed = handle_key(k, &mut self.state);
        crate::tui::scenarios::KeyOutcome { consumed, effects: Vec::new() }
    }

    fn keymap_entries(
        &self,
        _config: &crate::tui::workspace::ColumnConfig,
    ) -> Vec<(String, String)> {
        vec![
            ("↑ / ↓".into(), "move row cursor".into()),
            ("← / →".into(), "collapse / expand focused node".into()),
            ("Home / End".into(), "jump to top / bottom".into()),
            ("Space".into(), "toggle focused node".into()),
            ("m".into(), "toggle chat detail mode".into()),
            ("Tab / Shift-Tab".into(), "cycle body focus".into()),
        ]
    }
}

/// Decode a primitive-key synthetic NodeId of the form `{node}__p{i}` back
/// into the underlying node id. Returns `None` for non-primitive ids.
fn decode_prim_id(nid: &NodeId) -> Option<NodeId> {
    let s = &nid.0;
    let idx = s.rfind("__p")?;
    let (head, tail) = s.split_at(idx);
    // tail starts with "__p"; the suffix after must parse as usize.
    tail[3..].parse::<usize>().ok()?;
    Some(NodeId(head.to_string()))
}

fn decode_prim_index(nid: &NodeId) -> Option<usize> {
    let s = &nid.0;
    let idx = s.rfind("__p")?;
    s[idx + 3..].parse::<usize>().ok()
}
