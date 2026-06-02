//! Tool-detail scenario — the terminal port of the web `ToolDetailScenario`.
//!
//! Renders a tool-call detail column with specialized layouts for well-known
//! tool kinds (`edit`/`write`, `read`, `task`, `read_agent`) and a generic
//! fallback, plus a dedicated external-tool body. The body is composed as a
//! flat, vertically-scrolled list of styled lines; every long blob is rendered
//! through a *searchable body block* (the terminal analog of wrapping each web
//! `TextBlock searchable`), so the per-block `/` search affordance and the
//! span-level `external_query` highlight work everywhere.
//!
//! ## Layout (top → bottom)
//! collapsible metadata panel → optional hero panel → args/result section →
//! collapsible raw-attributes JSON. Column scroll covers the whole.
//!
//! ## Key dispatch (column layer)
//! `Tab`/`Shift-Tab` fall through to the global column-focus cycle; `↑`/`↓`
//! scroll; `Home`/`End` jump; `Space` toggles the metadata panel or focused
//! JSON view; `/` activates search on the focused body block (then
//! character/`Enter`/`Esc` keys route into it).
//!
//! Source for (shared `frontend/llr/`):
//! - `Tool detail requires tool call projection`
//! - `Tool detail prefers native tool call over external`
//! - `Tool detail empty state when no content captured`
//! - `Tool detail body blocks wrap in TextBlock for search`
//! - `Detail columns pass span search query to TextBlocks`
//! - `Tool detail metadata panel collapsible`
//! - `Tool detail hero panel surfaces key argument`
//! - `External tool detail body header fields`
//!
//! Also source for (new `frontend/tui/llr/`):
//! - `TUI Tool detail bottom-up layout in cells`
//! - `TUI Tool detail key-dispatch precedence within column`
//! - `TUI Tool detail metadata panel default closed`
//! - `TUI Tool detail empty state verbatim copy`

pub mod content;
pub mod dispatch;
pub mod hero;
pub mod render_edit;
pub mod render_external;
pub mod render_generic;
pub mod render_read_agent;
pub mod render_task;
pub mod render_view;
pub mod udiff;

use std::collections::HashMap;

use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use serde_json::Value;

use crate::tui::model::SpanDetail;
use crate::tui::widgets::searchable_text_block::{
    locate_matches, wrap_text, SearchPhase, SearchableTextBlock, SearchableTextBlockState,
};

/// The verbatim empty state shown when the resolved span has no tool-call
/// projection (required by `Tool detail requires tool call projection`).
pub const NOT_A_TOOL_LINE: &str = "selected span is not a tool call";

/// The verbatim empty state shown when a tool span captured no args/result
/// (required by `Tool detail empty state when no content captured`).
pub const NO_CONTENT_LINE: &str = "no content captured — set OTEL_INSTRUMENTATION_GENAI_CAPTURE_MESSAGE_CONTENT=true and OTEL_SEMCONV_STABILITY_OPT_IN=gen_ai_latest_experimental";

/// What `Space` / `/` mean for a given focusable block.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusKind {
    /// Collapsible metadata panel — `Space` toggles open/closed.
    Metadata,
    /// Collapsible JSON view — `Space` toggles; `/` searches when open.
    Json,
    /// Searchable body block — `/` activates search.
    Search,
}

/// Per-column tool-detail state. Owned by [`crate::tui::app::App`].
#[derive(Debug, Clone, Default)]
pub struct ToolDetailState {
    /// Whether the metadata panel is expanded. Closed by default.
    pub metadata_open: bool,
    /// Whether the raw-attributes JSON view is expanded. Closed by default.
    pub raw_attrs_open: bool,
    /// Column vertical scroll offset (top visible body line).
    pub scroll_top: u16,
    /// Index of the focused block within [`Self::focus_plan`].
    pub focused_block: usize,
    /// Per-searchable-block state, keyed by a stable block key.
    pub text_blocks: HashMap<String, SearchableTextBlockState>,
    /// Focus plan captured by the last render (key + kind, in block order).
    /// Read by [`handle_key`] for `Space`/`/` dispatch.
    pub focus_plan: Vec<(String, FocusKind)>,
    /// Total body line count from the last render (for scroll clamping).
    pub last_body_len: u16,
    /// Visible body height from the last render (for scroll clamping).
    pub last_view_h: u16,
    /// External (span-level) search query captured by the last render. Used by
    /// [`handle_key`] so per-block search input is suppressed while an external
    /// query drives the highlight.
    pub last_ext_query: Option<String>,
}

impl ToolDetailState {
    fn clamp_focus(&mut self) {
        let n = self.focus_plan.len();
        if n == 0 {
            self.focused_block = 0;
        } else if self.focused_block >= n {
            self.focused_block = n - 1;
        }
    }

    fn max_scroll(&self) -> u16 {
        self.last_body_len.saturating_sub(self.last_view_h)
    }
}

// ---------------------------------------------------------------------------
// Searchable line builder
// ---------------------------------------------------------------------------

/// Build the rendered lines for one searchable body block.
///
/// Reconciles the block's search phase against `external_query`/`focused`,
/// computes matches, and paints each glyph as `base_style` (optional syntect /
/// diff coloring) with a yellow match overlay (orange for the current match).
/// When `gutter` is `Some`, each logical source line gets a dim right-aligned
/// line-number prefix and the body is rendered one visual row per logical line
/// (truncated to width) so the gutter stays aligned; otherwise the body
/// soft-wraps at `width`.
///
/// Returns the lines plus the block-relative row of the current match (for
/// scroll-into-view), if any.
#[allow(clippy::too_many_arguments)]
fn build_block_lines(
    text: &str,
    width: u16,
    base_styles: Option<&[Style]>,
    gutter: Option<&[Option<u64>]>,
    st: &mut SearchableTextBlockState,
    external_query: Option<&str>,
    focused: bool,
) -> (Vec<Line<'static>>, Option<u16>) {
    SearchableTextBlock::reconcile(st, external_query, focused);
    let active = st.phase == SearchPhase::Active;

    // Match byte-ranges (row/col coords are unused here — we overlay by byte
    // offset so both wrap and gutter modes share the same logic).
    let matches = if active {
        locate_matches(text, &st.query, width.max(1))
    } else {
        Vec::new()
    };
    st.match_count = matches.len();
    if st.match_count == 0 {
        st.match_index = 0;
    } else if st.match_index >= st.match_count {
        st.match_index = st.match_count - 1;
    }
    let cur_range = matches
        .get(st.match_index)
        .map(|m| (m.byte_offset, m.byte_offset + m.byte_len));
    let in_match = |off: usize| -> bool {
        matches
            .iter()
            .any(|m| off >= m.byte_offset && off < m.byte_offset + m.byte_len)
    };
    let is_current = |off: usize| -> bool {
        cur_range.map(|(s, e)| off >= s && off < e).unwrap_or(false)
    };
    let style_for = |off: usize| -> Style {
        let base = base_styles.and_then(|s| s.get(off).copied()).unwrap_or_default();
        if is_current(off) {
            Style::default()
                .bg(Color::Rgb(255, 200, 0))
                .fg(Color::Black)
                .add_modifier(Modifier::BOLD)
        } else if in_match(off) {
            Style::default().bg(Color::Yellow).fg(Color::Black)
        } else {
            base
        }
    };

    let mut out: Vec<Line<'static>> = Vec::new();
    let mut header_rows: u16 = 0;
    if active {
        out.push(header_line(st));
        header_rows += 1;
    }

    let mut match_block_row: Option<u16> = None;

    match gutter {
        Some(gut) => {
            // Gutter width = widest line number.
            let gw = gut
                .iter()
                .filter_map(|n| n.map(|v| v.to_string().len()))
                .max()
                .unwrap_or(0);
            let code_w = width.saturating_sub(gw as u16 + 1).max(1) as usize;
            // Logical lines carry their starting byte offset for overlay.
            let mut byte = 0usize;
            for (i, logical) in text.split('\n').enumerate() {
                let mut spans: Vec<Span<'static>> = Vec::new();
                let num = gut.get(i).copied().flatten();
                let label = match num {
                    Some(n) => format!("{:>w$} ", n, w = gw),
                    None => format!("{:>w$} ", "", w = gw),
                };
                spans.push(Span::styled(
                    label,
                    Style::default().fg(Color::DarkGray).add_modifier(Modifier::DIM),
                ));
                push_glyph_spans(&mut spans, logical, byte, code_w, &style_for);
                if cur_range
                    .map(|(s, e)| {
                        let end = byte + logical.len();
                        s < end && e > byte
                    })
                    .unwrap_or(false)
                {
                    match_block_row = Some(out.len() as u16);
                }
                out.push(Line::from(spans));
                byte += logical.len() + 1; // + '\n'
            }
        }
        None => {
            let rows = wrap_text(text, width.max(1));
            for row in &rows {
                let mut spans: Vec<Span<'static>> = Vec::new();
                let mut run: Vec<char> = Vec::new();
                let mut run_style = Style::default();
                let mut first = true;
                let mut row_has_cur = false;
                for g in &row.glyphs {
                    let s = style_for(g.byte_offset);
                    if is_current(g.byte_offset) {
                        row_has_cur = true;
                    }
                    if first {
                        run_style = s;
                        first = false;
                    } else if s != run_style {
                        spans.push(Span::styled(run.iter().collect::<String>(), run_style));
                        run.clear();
                        run_style = s;
                    }
                    run.push(g.ch);
                }
                if !run.is_empty() {
                    spans.push(Span::styled(run.iter().collect::<String>(), run_style));
                }
                if row_has_cur {
                    match_block_row = Some(out.len() as u16);
                }
                out.push(Line::from(spans));
            }
        }
    }

    if active {
        out.push(input_line(st));
    }
    let _ = header_rows;
    (out, match_block_row)
}

/// Append up to `max_cols` styled glyph-spans for one logical (newline-free)
/// line starting at byte offset `base_byte`.
fn push_glyph_spans<F: Fn(usize) -> Style>(
    spans: &mut Vec<Span<'static>>,
    line: &str,
    base_byte: usize,
    max_cols: usize,
    style_for: &F,
) {
    let mut run: Vec<char> = Vec::new();
    let mut run_style = Style::default();
    let mut first = true;
    for (cols, (off, ch)) in line.char_indices().enumerate() {
        if cols >= max_cols {
            break;
        }
        let s = style_for(base_byte + off);
        if first {
            run_style = s;
            first = false;
        } else if s != run_style {
            spans.push(Span::styled(run.iter().collect::<String>(), run_style));
            run.clear();
            run_style = s;
        }
        run.push(ch);
    }
    if !run.is_empty() {
        spans.push(Span::styled(run.iter().collect::<String>(), run_style));
    }
}

fn header_line(st: &SearchableTextBlockState) -> Line<'static> {
    let txt = if !st.query.is_empty() && st.match_count > 0 {
        format!("  {} of {} matches", st.match_index + 1, st.match_count)
    } else {
        "  0 matches".to_string()
    };
    Line::from(Span::styled(
        txt,
        Style::default().fg(Color::DarkGray).add_modifier(Modifier::DIM),
    ))
}

fn input_line(st: &SearchableTextBlockState) -> Line<'static> {
    Line::from(Span::styled(
        format!("  / {}", st.query),
        Style::default().fg(Color::Cyan),
    ))
}

// ---------------------------------------------------------------------------
// Body assembly context
// ---------------------------------------------------------------------------

/// Accumulator threaded through the renderers. Pushes styled lines and records
/// the focus plan as blocks are emitted.
pub(crate) struct BodyCtx<'a> {
    lines: Vec<Line<'static>>,
    focus_plan: Vec<(String, FocusKind)>,
    width: u16,
    ext: Option<&'a str>,
    col_focused: bool,
    focused_block: usize,
    text_blocks: &'a mut HashMap<String, SearchableTextBlockState>,
    metadata_open: bool,
    raw_attrs_open: bool,
    scroll_target: Option<u16>,
}

impl<'a> BodyCtx<'a> {
    fn is_focused(&self, block_idx: usize) -> bool {
        self.col_focused && self.focused_block == block_idx
    }

    /// A dim section label line.
    pub(crate) fn label(&mut self, text: &str) {
        self.lines.push(Line::from(Span::styled(
            text.to_string(),
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        )));
    }

    /// A small dim sub-label line.
    pub(crate) fn sublabel(&mut self, text: &str) {
        self.lines.push(Line::from(Span::styled(
            text.to_string(),
            Style::default().fg(Color::DarkGray),
        )));
    }

    /// A blank spacer line.
    pub(crate) fn gap(&mut self) {
        self.lines.push(Line::from(""));
    }

    /// A single key/value chip row (not searchable).
    pub(crate) fn kv_row(&mut self, k: &str, v: &str) {
        let spans = vec![
            Span::styled(
                format!("{k}: "),
                Style::default().fg(Color::DarkGray),
            ),
            Span::styled(v.to_string(), Style::default().fg(Color::White)),
        ];
        self.lines.push(Line::from(spans));
    }

    /// Push a searchable body block; returns nothing but records the focus
    /// entry. `base_styles`/`gutter` are optional decorations.
    pub(crate) fn search_block(
        &mut self,
        key: &str,
        text: &str,
        base_styles: Option<Vec<Style>>,
        gutter: Option<&[Option<u64>]>,
    ) {
        let block_idx = self.focus_plan.len();
        let focused = self.is_focused(block_idx);
        let block_start = self.lines.len() as u16;
        let st = self.text_blocks.entry(key.to_string()).or_default();
        let (mut block_lines, match_row) = build_block_lines(
            text,
            self.width,
            base_styles.as_deref(),
            gutter,
            st,
            self.ext,
            focused,
        );
        if focused {
            if let Some(mr) = match_row {
                self.scroll_target = Some(block_start + mr);
            } else {
                self.scroll_target = Some(block_start);
            }
        }
        self.focus_plan.push((key.to_string(), FocusKind::Search));
        self.lines.append(&mut block_lines);
    }

    /// A searchable code block highlighted via syntect for `lang`.
    pub(crate) fn code_block(&mut self, key: &str, text: &str, lang: Option<&str>) {
        let styles = crate::tui::widgets::code_block::syntect_byte_styles(text, lang);
        self.search_block(key, text, styles, None);
    }

    /// A searchable JSON body block (pretty-printed text).
    pub(crate) fn json_text(&mut self, key: &str, value: &Value) {
        let pretty = crate::tui::widgets::json_view::JsonView::pretty(value);
        self.search_block(key, &pretty, None, None);
    }

    /// Markdown body — rendered to styled lines and routed through
    /// [`search_block`](Self::search_block) so the block participates in
    /// per-block `/` search and receives the column `external_query`. Markdown
    /// styling becomes the base style under which match highlights paint.
    pub(crate) fn markdown(&mut self, key: &str, md: &str) {
        // Render unwrapped (width 0) so search_block performs the single,
        // authoritative wrap at `self.width`. Flatten the styled lines into a
        // text buffer plus one base `Style` per byte (default for the `\n`
        // join bytes), preserving the `styles.len() == text.len()` invariant.
        let lines = crate::tui::widgets::markdown::markdown_to_lines(md, 0);
        let mut text = String::new();
        let mut styles: Vec<Style> = Vec::new();
        for (i, line) in lines.iter().enumerate() {
            if i > 0 {
                text.push('\n');
                styles.push(Style::default());
            }
            for span in &line.spans {
                let blen = span.content.len();
                text.push_str(&span.content);
                styles.extend(std::iter::repeat_n(span.style, blen));
            }
        }
        debug_assert_eq!(styles.len(), text.len());
        self.search_block(key, &text, Some(styles), None);
    }

    /// The verbatim no-content empty state.
    pub(crate) fn no_content(&mut self) {
        self.lines.push(Line::from(Span::styled(
            NO_CONTENT_LINE.to_string(),
            Style::default().fg(Color::DarkGray).add_modifier(Modifier::ITALIC),
        )));
    }

    /// A collapsible metadata panel (closed shows just the summary).
    pub(crate) fn metadata_panel(&mut self, tool_name: &str, kv: &[(String, String)]) {
        let block_idx = self.focus_plan.len();
        let focused = self.is_focused(block_idx);
        let glyph = if self.metadata_open { "▾" } else { "▸" };
        let mut summary_style = Style::default().fg(Color::White).add_modifier(Modifier::BOLD);
        if focused {
            summary_style = summary_style.bg(Color::Rgb(40, 40, 40));
        }
        self.lines.push(Line::from(Span::styled(
            format!("{glyph} {tool_name}"),
            summary_style,
        )));
        if self.metadata_open {
            for (k, v) in kv {
                self.kv_row(k, v);
            }
        }
        if focused {
            self.scroll_target = Some(block_idx_line_hint(&self.lines));
        }
        self.focus_plan.push((tool_name.to_string(), FocusKind::Metadata));
    }

    /// A collapsible raw-attributes JSON view.
    pub(crate) fn json_panel(&mut self, key: &str, value: &Value) {
        let block_idx = self.focus_plan.len();
        let focused = self.is_focused(block_idx);
        let glyph = if self.raw_attrs_open { "▾" } else { "▸" };
        let mut summary_style = Style::default().fg(Color::DarkGray).add_modifier(Modifier::DIM);
        if focused {
            summary_style = summary_style.bg(Color::Rgb(40, 40, 40));
        }
        let summary_start = self.lines.len() as u16;
        self.lines.push(Line::from(Span::styled(
            format!("{glyph} json…"),
            summary_style,
        )));
        if self.raw_attrs_open {
            let pretty = crate::tui::widgets::json_view::JsonView::pretty(value);
            let st = self.text_blocks.entry(key.to_string()).or_default();
            let (mut block_lines, match_row) =
                build_block_lines(&pretty, self.width, None, None, st, self.ext, focused);
            if focused {
                self.scroll_target = Some(summary_start + match_row.map(|r| r + 1).unwrap_or(0));
            }
            self.lines.append(&mut block_lines);
        } else if focused {
            self.scroll_target = Some(summary_start);
        }
        self.focus_plan.push((key.to_string(), FocusKind::Json));
    }
}

fn block_idx_line_hint(lines: &[Line<'static>]) -> u16 {
    // Scroll target for a metadata panel = its summary line (first of block).
    // The summary was just pushed; if a kv grid followed, the summary is a few
    // lines back, but bringing the summary into view is enough.
    lines.len().saturating_sub(1) as u16
}

// ---------------------------------------------------------------------------
// Public render
// ---------------------------------------------------------------------------

/// Render the tool-detail column body into `area`.
///
/// Reads `config.{selected_trace_id, selected_span_id, search_query}` and the
/// cached span detail. `cached_detail` is the resolved
/// [`crate::tui::app::App::cached_span_detail`] for the configured selection
/// (the caller resolves it so this module stays decoupled from `App`).
pub fn render(
    area: Rect,
    buf: &mut Buffer,
    state: &mut ToolDetailState,
    selection: Option<(&str, &str)>,
    search_query: Option<&str>,
    cached_detail: Option<&SpanDetail>,
    col_focused: bool,
) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let ext = search_query.filter(|s| !s.is_empty());
    state.last_ext_query = ext.map(|s| s.to_string());

    // Empty / loading states (no focusable blocks).
    let lines: Vec<Line<'static>> = match (selection, cached_detail) {
        (None, _) => {
            state.focus_plan.clear();
            empty_lines("Select a tool span in the Spans column.")
        }
        (Some(_), None) => {
            state.focus_plan.clear();
            empty_lines("loading…")
        }
        (Some(_), Some(detail)) => {
            state.clamp_focus();
            let mut ctx = BodyCtx {
                lines: Vec::new(),
                focus_plan: Vec::new(),
                width: area.width,
                ext,
                col_focused,
                focused_block: state.focused_block,
                text_blocks: &mut state.text_blocks,
                metadata_open: state.metadata_open,
                raw_attrs_open: state.raw_attrs_open,
                scroll_target: None,
            };
            build_body(&mut ctx, detail);
            let scroll_target = ctx.scroll_target;
            let lines = ctx.lines;
            state.focus_plan = ctx.focus_plan;
            state.clamp_focus();

            // Scroll-into-view for the focused active match / block.
            let view_h = area.height;
            let max_scroll = (lines.len() as u16).saturating_sub(view_h);
            if let Some(target) = scroll_target {
                if target < state.scroll_top {
                    state.scroll_top = target;
                } else if view_h > 0 && target >= state.scroll_top + view_h {
                    state.scroll_top = target.saturating_sub(view_h.saturating_sub(1));
                }
            }
            if state.scroll_top > max_scroll {
                state.scroll_top = max_scroll;
            }
            lines
        }
    };

    state.last_body_len = lines.len() as u16;
    state.last_view_h = area.height;
    if state.scroll_top > state.max_scroll() {
        state.scroll_top = state.max_scroll();
    }

    // Blit the visible window.
    let start = state.scroll_top as usize;
    for (vis, line) in lines.iter().skip(start).take(area.height as usize).enumerate() {
        let y = area.y + vis as u16;
        buf.set_line(area.x, y, line, area.width);
    }
}

fn empty_lines(text: &str) -> Vec<Line<'static>> {
    vec![Line::from(Span::styled(
        text.to_string(),
        Style::default().fg(Color::DarkGray).add_modifier(Modifier::ITALIC),
    ))]
}

/// Assemble the full body for a resolved span.
fn build_body(ctx: &mut BodyCtx, detail: &SpanDetail) {
    use dispatch::{route, RendererKind, Route};
    match route(detail) {
        Route::Empty(_) => {
            ctx.lines.push(Line::from(Span::styled(
                NOT_A_TOOL_LINE.to_string(),
                Style::default().fg(Color::DarkGray).add_modifier(Modifier::ITALIC),
            )));
        }
        Route::External => render_external::render(ctx, detail),
        Route::Native(kind) => {
            render_native_header_and_hero(ctx, detail);
            ctx.gap();
            ctx.label("args / result");
            let attrs = detail.span.attributes.clone().unwrap_or(Value::Null);
            match kind {
                RendererKind::Edit => render_edit::render(ctx, &attrs),
                RendererKind::View => render_view::render(ctx, &attrs),
                RendererKind::Task => render_task::render(ctx, &attrs),
                RendererKind::ReadAgent => render_read_agent::render(ctx, &attrs),
                RendererKind::Generic | RendererKind::External => {
                    render_generic::render(ctx, &attrs)
                }
            }
            ctx.gap();
            ctx.label("raw span attributes");
            let raw = detail.span.attributes.clone().unwrap_or(Value::Null);
            ctx.json_panel("raw_attrs", &raw);
        }
    }
}

/// Native metadata panel + optional hero panel.
fn render_native_header_and_hero(ctx: &mut BodyCtx, detail: &SpanDetail) {
    use crate::tui::format::{fmt_clock, fmt_ns};
    use crate::tui::scenarios::spans::attrs::parse_tool_call_arguments;

    let tc = detail
        .projection
        .tool_call
        .as_ref()
        .expect("native route implies tool_call");
    let span = &detail.span;
    let dur = span.duration_ns.map(|d| d as i128).or_else(|| {
        match (span.start_unix_ns, span.end_unix_ns) {
            (Some(s), Some(e)) => Some(e - s),
            _ => None,
        }
    });
    let tool_name = tc.tool_name.clone().unwrap_or_else(|| "(unknown tool)".to_string());
    let kv = vec![
        ("call_id".to_string(), tc.call_id.clone().unwrap_or_else(|| "—".to_string())),
        ("tool_type".to_string(), tc.tool_type.clone().unwrap_or_else(|| "—".to_string())),
        ("duration".to_string(), fmt_ns(dur)),
        (
            "status".to_string(),
            tc.status_code.map(|c| c.to_string()).unwrap_or_else(|| "—".to_string()),
        ),
        ("start".to_string(), fmt_clock(span.start_unix_ns)),
        (
            "conv".to_string(),
            tc.conversation_id
                .as_ref()
                .map(|c| c.chars().take(8).collect::<String>())
                .unwrap_or_else(|| "—".to_string()),
        ),
    ];
    ctx.metadata_panel(&tool_name, &kv);

    // Hero panel: only for function tool_type.
    let attrs = span.attributes.clone().unwrap_or(Value::Null);
    let args = parse_tool_call_arguments(&attrs).unwrap_or(Value::Null);
    if let Some(value) = hero::hero_value(&args, tc.tool_type.as_deref()) {
        ctx.gap();
        for line in value.lines() {
            ctx.lines.push(Line::from(Span::styled(
                line.to_string(),
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
            )));
        }
    }
}

// ---------------------------------------------------------------------------
// Key dispatch
// ---------------------------------------------------------------------------

/// Handle a column-layer key. Returns `true` if consumed.
pub fn handle_key(key: KeyEvent, state: &mut ToolDetailState) -> bool {
    state.clamp_focus();
    let ext = state.last_ext_query.clone();
    let ext_ref = ext.as_deref();

    // Text-input precedence: an active search on the focused block consumes
    // editing/navigation keys first.
    if let Some((k, kind)) = state.focus_plan.get(state.focused_block).cloned() {
        let searchable_open = matches!(kind, FocusKind::Search)
            || (matches!(kind, FocusKind::Json) && state.raw_attrs_open);
        if searchable_open {
            if let Some(st) = state.text_blocks.get_mut(&k) {
                if st.phase == SearchPhase::Active
                    && SearchableTextBlock::handle_key(key, st, ext_ref)
                {
                    return true;
                }
            }
        }
    }

    match (key.code, key.modifiers) {
        (KeyCode::Tab | KeyCode::BackTab, _) => false,
        (KeyCode::Up, _) => {
            state.scroll_top = state.scroll_top.saturating_sub(1);
            true
        }
        (KeyCode::Down, _) => {
            state.scroll_top = (state.scroll_top + 1).min(state.max_scroll());
            true
        }
        (KeyCode::Home, _) => {
            state.scroll_top = 0;
            true
        }
        (KeyCode::End, _) => {
            state.scroll_top = state.max_scroll();
            true
        }
        (KeyCode::Char(' '), m) if !m.contains(KeyModifiers::CONTROL) => {
            match state.focus_plan.get(state.focused_block).map(|(_, k)| *k) {
                Some(FocusKind::Metadata) => {
                    state.metadata_open = !state.metadata_open;
                    true
                }
                Some(FocusKind::Json) => {
                    state.raw_attrs_open = !state.raw_attrs_open;
                    true
                }
                _ => false,
            }
        }
        (KeyCode::Char('/'), _) => {
            if let Some((k, kind)) = state.focus_plan.get(state.focused_block).cloned() {
                let can_search = matches!(kind, FocusKind::Search)
                    || (matches!(kind, FocusKind::Json) && state.raw_attrs_open);
                if can_search {
                    let st = state.text_blocks.entry(k).or_default();
                    return SearchableTextBlock::handle_key(key, st, ext_ref);
                }
            }
            false
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests;
