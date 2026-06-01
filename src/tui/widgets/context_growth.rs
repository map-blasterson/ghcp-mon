//! Context Growth Widget — a per-turn context-token stacked bar chart pinned
//! to the bottom of the workspace.
//!
//! Source for (shared `frontend/llr/`):
//! - `Context widget chart visual styling`
//! - `Context widget stack chart per turn`
//! - `Context widget cache read green segment`
//! - `Context widget colors sub-agent input bar distinctly`
//! - `Context widget hovered chat highlights matching column`
//! - `Context widget hide and show toggle` (collapsed bar)
//!
//! and (new `frontend/tui/llr/`):
//! - `TUI Context widget bar-cell mapping rules`
//! - `TUI Context widget legend layout`
//! - `TUI Context widget collapsed bar layout`
//! - `TUI Context widget keyboard bar cursor`
//!
//! The pure snapshot-merge logic lives in [`merge`]; this module is the
//! renderer plus the small render-time geometry helpers shared with the
//! `app` key handlers so bar indices line up between draw and input.

pub mod merge;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Widget};

use crate::tui::format::fmt_compact_count;

pub use merge::{
    MergedRow, chat_span_pks, max_current_tokens, merge_snapshots, split_segments,
};

// ---- Palette (24-bit RGB so it matches the web hex values exactly) ----

/// `cache_read` segment — green `#4ade80`.
pub const CACHE_READ: Color = Color::Rgb(0x4a, 0xde, 0x80);
/// Fresh `input` (root agent) — blue `#60a5fa`.
pub const INPUT_ROOT: Color = Color::Rgb(0x60, 0xa5, 0xfa);
/// Fresh `input` (sub-agent) — lighter blue `#93c5fd`.
pub const INPUT_SUB: Color = Color::Rgb(0x93, 0xc5, 0xfd);
/// `output` segment — orange `#fb923c`.
pub const OUTPUT: Color = Color::Rgb(0xfb, 0x92, 0x3c);
/// `reasoning` segment — yellow `#fde047`.
pub const REASONING: Color = Color::Rgb(0xfd, 0xe0, 0x47);
/// Limit line / hover underbar — yellow.
pub const LIMIT_YELLOW: Color = Color::Rgb(0xfd, 0xe0, 0x47);

/// Width of one bar in terminal cells.
pub const BAR_CELL_WIDTH: u16 = 1;
/// Cell stride between successive bars (bar + 1-cell gap).
pub const BAR_STRIDE: u16 = 2;
/// Cells reserved on the left for the y-axis tick labels.
pub const Y_AXIS_W: u16 = 9;

/// Bundle of merged rows plus the y-axis occupancy anchor. `max_current_tokens`
/// is taken from the raw `usage_info_event` snapshots (it is not a per-row
/// field) and is needed to anchor the y-axis per `Context widget stack chart
/// per turn`.
#[derive(Debug, Clone, Default)]
pub struct MergedRows {
    pub rows: Vec<MergedRow>,
    pub max_current_tokens: i64,
}

impl MergedRows {
    /// Maximum non-null `token_limit` across rows (zero if none reported).
    pub fn max_token_limit(&self) -> i64 {
        self.rows.iter().filter_map(|r| r.token_limit).max().unwrap_or(0)
    }

    /// Y-axis maximum: `1.10 * max(maxTokenLimit, maxCurrent)`, falling back to
    /// `1` only when both are zero.
    pub fn y_max(&self) -> f64 {
        let m = self.max_token_limit().max(self.max_current_tokens);
        if m <= 0 { 1.0 } else { 1.10 * m as f64 }
    }
}

/// Stateful render bits (the keyboard analog of mouse hover).
#[derive(Debug, Default, Clone)]
pub struct ContextGrowthState {
    /// Index (into the merged-row list) of the bar under the keyboard cursor.
    pub bar_cursor: Option<usize>,
    /// Number of bars rendered last frame (kept for hover-sync calculations).
    pub last_visible_rows: u16,
}

/// Resolved plot geometry. Both the renderer and the `app` key handlers derive
/// bar positions from this so `bar_cursor` indices map to the same columns.
#[derive(Debug, Clone, Copy)]
pub struct PlotGeom {
    pub plot_x0: u16,
    pub plot_width: u16,
    pub top_row: u16,
    pub baseline_row: u16,
    pub plot_h: u16,
    pub underbar_row: u16,
    pub max_bars: usize,
}

/// Compute plot geometry for a widget `area`. Returns `None` when the area is
/// too short to host a header row, plot rows, and an underbar row.
pub fn plot_geometry(area: Rect) -> Option<PlotGeom> {
    if area.height < 3 || area.width <= Y_AXIS_W {
        return None;
    }
    let plot_x0 = area.x + Y_AXIS_W;
    let plot_width = area.width - Y_AXIS_W;
    let top_row = area.y + 1;
    let underbar_row = area.y + area.height - 1;
    let baseline_row = underbar_row - 1;
    let plot_h = baseline_row - top_row + 1;
    let max_bars = ((plot_width as usize) + 1) / (BAR_STRIDE as usize);
    Some(PlotGeom {
        plot_x0,
        plot_width,
        top_row,
        baseline_row,
        plot_h,
        underbar_row,
        max_bars,
    })
}

/// Stateless renderer. Construct per-frame with borrowed data.
pub struct ContextGrowthWidget<'a> {
    pub data: &'a MergedRows,
    pub session_id: Option<&'a str>,
    pub hovered_chat_pk: Option<i64>,
}

impl<'a> ContextGrowthWidget<'a> {
    /// Render the collapsed single-line `▾ context growth` bar
    /// (`Context widget hide and show toggle`). Painted when the widget is not
    /// visible.
    pub fn render_collapsed(area: Rect, buf: &mut Buffer) {
        if area.height == 0 {
            return;
        }
        let line = Line::from(Span::styled(
            "▾ context growth  (press c to expand)",
            Style::default().fg(Color::DarkGray).add_modifier(Modifier::DIM),
        ));
        let row = Rect::new(area.x, area.y, area.width, 1);
        Paragraph::new(line).render(row, buf);
    }

    /// Render the chart. `state.bar_cursor` paints the keyboard-cursor
    /// underbar; `hovered_chat_pk` paints the cross-column hover underbar.
    pub fn render(&self, area: Rect, buf: &mut Buffer, state: &ContextGrowthState) {
        // Header / legend row.
        self.render_header(area, buf);

        let Some(geom) = plot_geometry(area) else {
            return;
        };
        let y_max = self.data.y_max();
        let plot_h = geom.plot_h as f64;
        let cells = |v: i64| -> f64 { (v.max(0) as f64 / y_max) * plot_h };

        // Y-axis tick labels (top = y_max, middle, baseline = 0).
        self.render_y_axis(area, buf, &geom, y_max);

        // Dashed limit line first, so bars render on top of it where they
        // overlap (the line is context, the bars are the data).
        self.render_limit_line(buf, &geom, &cells);

        // One stacked bar per merged row, head-truncated to what fits.
        let n = self.data.rows.len().min(geom.max_bars);
        for (i, row) in self.data.rows.iter().take(n).enumerate() {
            let x = geom.plot_x0 + (i as u16) * BAR_STRIDE;
            if x >= geom.plot_x0 + geom.plot_width {
                break;
            }
            self.render_bar(buf, &geom, x, row, &cells);

            // Underbar: keyboard cursor OR cross-column hover.
            let cursor_hit = state.bar_cursor == Some(i);
            let hover_hit = self.hovered_chat_pk == Some(row.span_pk);
            if cursor_hit || hover_hit {
                if let Some(cell) = buf.cell_mut((x, geom.underbar_row)) {
                    cell.set_symbol("▔")
                        .set_style(Style::default().fg(LIMIT_YELLOW).add_modifier(Modifier::BOLD));
                }
            }
        }
    }

    fn render_bar(
        &self,
        buf: &mut Buffer,
        geom: &PlotGeom,
        x: u16,
        row: &MergedRow,
        cells: &impl Fn(i64) -> f64,
    ) {
        let (cache_r, fresh, out, rea) = split_segments(row);
        let total = cache_r + fresh + out + rea;
        if total == 0 {
            return; // `Context widget cache read green segment`: no sub-bars.
        }
        let input_color = if row.is_sub_agent { INPUT_SUB } else { INPUT_ROOT };
        // Stacked cumulative boundaries (in cell units) from the baseline up.
        let c1 = cells(cache_r);
        let c2 = cells(cache_r + fresh);
        let c3 = cells(cache_r + fresh + out);
        let c4 = cells(total);
        for rr in 0..geom.plot_h {
            let center = rr as f64 + 0.5;
            let color = if center < c1 {
                CACHE_READ
            } else if center < c2 {
                input_color
            } else if center < c3 {
                OUTPUT
            } else if center < c4 {
                REASONING
            } else {
                continue; // above the stack (clips at plot top when total > y_max).
            };
            let y = geom.baseline_row - rr;
            if let Some(cell) = buf.cell_mut((x, y)) {
                cell.set_symbol("█").set_style(Style::default().fg(color));
            }
        }
    }

    fn render_limit_line(
        &self,
        buf: &mut Buffer,
        geom: &PlotGeom,
        cells: &impl Fn(i64) -> f64,
    ) {
        let max_limit = self.data.max_token_limit();
        if max_limit <= 0 {
            return;
        }
        let lc = cells(max_limit);
        if lc < 0.0 || lc >= geom.plot_h as f64 {
            return; // limit above the plot top — nothing to draw.
        }
        let lr = lc.round() as u16;
        let y = geom.baseline_row - lr.min(geom.plot_h.saturating_sub(1));
        let style = Style::default().fg(LIMIT_YELLOW).add_modifier(Modifier::DIM);
        for dx in 0..geom.plot_width {
            if dx % 2 == 0 {
                let x = geom.plot_x0 + dx;
                if let Some(cell) = buf.cell_mut((x, y)) {
                    cell.set_symbol("─").set_style(style);
                }
            }
        }
        // "context limit" label at the right, with a solid bg mask.
        let label = " context limit ";
        let lw = label.chars().count() as u16;
        if geom.plot_width > lw {
            let lx = geom.plot_x0 + geom.plot_width - lw;
            buf.set_string(
                lx,
                y,
                label,
                Style::default()
                    .fg(LIMIT_YELLOW)
                    .bg(Color::Reset)
                    .add_modifier(Modifier::BOLD),
            );
        }
    }

    fn render_y_axis(&self, area: Rect, buf: &mut Buffer, geom: &PlotGeom, y_max: f64) {
        let mid_row = geom.top_row + geom.plot_h / 2;
        let ticks: [(u16, i64); 3] = [
            (geom.top_row, y_max.round() as i64),
            (mid_row, (y_max / 2.0).round() as i64),
            (geom.baseline_row, 0),
        ];
        let label_w = (Y_AXIS_W - 1) as usize;
        for (y, v) in ticks {
            let mut s = fmt_compact_count(v);
            if s.chars().count() > label_w {
                s = s.chars().take(label_w).collect();
            }
            // Right-align inside the y-axis gutter.
            let pad = label_w.saturating_sub(s.chars().count());
            let text = format!("{}{} ", " ".repeat(pad), s);
            buf.set_string(area.x, y, text, Style::default().fg(Color::DarkGray));
        }
    }

    fn render_header(&self, area: Rect, buf: &mut Buffer) {
        let mut spans: Vec<Span<'static>> = Vec::new();
        let legend: [(&str, Color); 5] = [
            ("cache", CACHE_READ),
            ("input", INPUT_ROOT),
            ("sub-input", INPUT_SUB),
            ("output", OUTPUT),
            ("reasoning", REASONING),
        ];
        for (label, color) in legend {
            spans.push(Span::styled("█", Style::default().fg(color)));
            spans.push(Span::styled(format!("{label} "), Style::default().fg(Color::Gray)));
        }
        // Limit swatch (dashed yellow).
        spans.push(Span::styled("┄", Style::default().fg(LIMIT_YELLOW)));
        spans.push(Span::styled("limit ", Style::default().fg(Color::Gray)));

        // Active session id + hide hint.
        let sess = match self.session_id {
            Some(s) if !s.is_empty() => {
                format!(" s:{}", s.chars().take(8).collect::<String>())
            }
            _ => " pick a session".to_string(),
        };
        spans.push(Span::styled(sess, Style::default().fg(Color::Cyan)));
        spans.push(Span::styled(
            " [c ✕]",
            Style::default().fg(Color::DarkGray),
        ));

        let row = Rect::new(area.x, area.y, area.width, 1);
        Paragraph::new(Line::from(spans)).render(row, buf);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::backend::TestBackend;
    use ratatui::{Terminal, layout::Rect};

    fn row(span_pk: i64, input: i64, cache: i64, out: i64, rea: i64, sub: bool, limit: Option<i64>) -> MergedRow {
        MergedRow {
            span_pk,
            token_limit: limit,
            input_tokens: Some(input),
            output_tokens: Some(out),
            reasoning_tokens: Some(rea),
            cache_read_tokens: Some(cache),
            latest_ns: 0,
            is_sub_agent: sub,
        }
    }

    fn render_to_string(
        data: &MergedRows,
        state: &ContextGrowthState,
        hovered: Option<i64>,
        session: Option<&str>,
        w: u16,
        h: u16,
    ) -> (String, Buffer) {
        let backend = TestBackend::new(w, h);
        let mut term = Terminal::new(backend).unwrap();
        term.draw(|f| {
            let area = Rect::new(0, 0, w, h);
            let widget = ContextGrowthWidget {
                data,
                session_id: session,
                hovered_chat_pk: hovered,
            };
            widget.render(area, f.buffer_mut(), state);
        })
        .unwrap();
        let buf = term.backend().buffer().clone();
        let mut joined = String::new();
        for y in 0..buf.area.height {
            for x in 0..buf.area.width {
                joined.push_str(buf[(x, y)].symbol());
            }
            joined.push('\n');
        }
        (joined, buf)
    }

    #[test]
    fn header_legend_words_render() {
        let data = MergedRows {
            rows: vec![row(1, 4000, 1000, 200, 50, false, Some(8000))],
            max_current_tokens: 5000,
        };
        let state = ContextGrowthState::default();
        let (text, _) = render_to_string(&data, &state, None, Some("abcdef0123"), 80, 12);
        for needle in ["cache", "input", "sub-input", "output", "reasoning", "limit"] {
            assert!(text.contains(needle), "missing legend '{needle}' in:\n{text}");
        }
        assert!(text.contains("s:abcdef01"), "missing session id in:\n{text}");
    }

    #[test]
    fn dashed_limit_line_renders() {
        let data = MergedRows {
            rows: vec![row(1, 4000, 1000, 200, 50, false, Some(8000))],
            max_current_tokens: 5000,
        };
        let state = ContextGrowthState::default();
        let (text, _) = render_to_string(&data, &state, None, Some("abc"), 80, 14);
        assert!(text.contains("─"), "missing dashed limit line in:\n{text}");
        assert!(text.contains("context limit"), "missing limit label in:\n{text}");
    }

    #[test]
    fn at_least_one_bar_cell_renders() {
        let data = MergedRows {
            rows: vec![row(1, 4000, 1000, 200, 50, false, Some(8000))],
            max_current_tokens: 5000,
        };
        let state = ContextGrowthState::default();
        let (text, buf) = render_to_string(&data, &state, None, Some("abc"), 80, 14);
        assert!(text.contains("█"), "missing bar cells in:\n{text}");
        // A green cache_read cell must exist at the baseline column.
        let geom = plot_geometry(Rect::new(0, 0, 80, 14)).unwrap();
        let cell = &buf[(geom.plot_x0, geom.baseline_row)];
        assert_eq!(cell.symbol(), "█");
        assert_eq!(cell.style().fg, Some(CACHE_READ));
    }

    #[test]
    fn bar_cursor_paints_underbar() {
        let data = MergedRows {
            rows: vec![row(1, 4000, 1000, 200, 50, false, Some(8000))],
            max_current_tokens: 5000,
        };
        let state = ContextGrowthState {
            bar_cursor: Some(0),
            last_visible_rows: 1,
        };
        let (text, buf) = render_to_string(&data, &state, None, Some("abc"), 80, 14);
        assert!(text.contains("▔"), "missing cursor underbar in:\n{text}");
        let geom = plot_geometry(Rect::new(0, 0, 80, 14)).unwrap();
        assert_eq!(buf[(geom.plot_x0, geom.underbar_row)].symbol(), "▔");
    }

    #[test]
    fn hovered_chat_pk_paints_underbar() {
        let data = MergedRows {
            rows: vec![
                row(1, 4000, 1000, 200, 50, false, Some(8000)),
                row(2, 3000, 500, 100, 20, false, Some(8000)),
            ],
            max_current_tokens: 5000,
        };
        let state = ContextGrowthState::default();
        // Hover the SECOND bar (span_pk 2).
        let (_text, buf) = render_to_string(&data, &state, Some(2), Some("abc"), 80, 14);
        let geom = plot_geometry(Rect::new(0, 0, 80, 14)).unwrap();
        let x = geom.plot_x0 + BAR_STRIDE; // second bar column
        assert_eq!(buf[(x, geom.underbar_row)].symbol(), "▔");
        // First bar has no underbar.
        assert_ne!(buf[(geom.plot_x0, geom.underbar_row)].symbol(), "▔");
    }

    #[test]
    fn sub_agent_input_uses_lighter_blue() {
        // A row with no cache so the fresh-input segment starts at baseline.
        let data = MergedRows {
            rows: vec![row(1, 4000, 0, 0, 0, true, Some(8000))],
            max_current_tokens: 5000,
        };
        let state = ContextGrowthState::default();
        let (_t, buf) = render_to_string(&data, &state, None, Some("abc"), 80, 14);
        let geom = plot_geometry(Rect::new(0, 0, 80, 14)).unwrap();
        let cell = &buf[(geom.plot_x0, geom.baseline_row)];
        assert_eq!(cell.style().fg, Some(INPUT_SUB));
    }

    #[test]
    fn collapsed_bar_renders_single_line() {
        let backend = TestBackend::new(40, 1);
        let mut term = Terminal::new(backend).unwrap();
        term.draw(|f| {
            ContextGrowthWidget::render_collapsed(Rect::new(0, 0, 40, 1), f.buffer_mut());
        })
        .unwrap();
        let buf = term.backend().buffer();
        let mut joined = String::new();
        for x in 0..buf.area.width {
            joined.push_str(buf[(x, 0)].symbol());
        }
        assert!(joined.contains("context growth"), "got: {joined}");
        assert!(joined.contains("▾"), "missing collapse glyph: {joined}");
    }

    #[test]
    fn no_session_shows_pick_a_session() {
        let data = MergedRows::default();
        let state = ContextGrowthState::default();
        let (text, _) = render_to_string(&data, &state, None, None, 80, 12);
        assert!(text.contains("pick a session"), "in:\n{text}");
    }

    #[test]
    fn bar_total_zero_renders_no_segments() {
        let data = MergedRows {
            rows: vec![row(1, 0, 0, 0, 0, false, Some(8000))],
            max_current_tokens: 8000,
        };
        let state = ContextGrowthState::default();
        let (_t, buf) = render_to_string(&data, &state, None, Some("abc"), 80, 14);
        let geom = plot_geometry(Rect::new(0, 0, 80, 14)).unwrap();
        // baseline column at the first bar must not be a bar cell.
        assert_ne!(buf[(geom.plot_x0, geom.baseline_row)].symbol(), "█");
    }
}
