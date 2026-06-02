//! Proportional segment bar — the Chat Detail summary row.
//!
//! ## Why custom buffer-paint (not Gauge / LineGauge / BarChart)
//!
//! After reading the ratatui docs:
//!   * `Gauge` and `LineGauge` render a *single* progress percentage; they
//!     can't paint N adjacent colored segments in one row.
//!   * `BarChart` renders multiple bars but each bar gets the same `bar_style`
//!     unless wrapped in `BarGroup` (and the heights map a numeric value to
//!     vertical block fill, not horizontal width).
//!   * Our model is "N visible-frontier nodes, each with its own bytes and
//!     its own hashed colour, painted as a single horizontal strip filling the
//!     summary row exactly". The closest precedent in this repo is
//!     `src/tui/widgets/context_growth.rs` which paints `█` cells directly via
//!     `Buffer::cell_mut` for the same reason.
//!
//! We therefore expose [`allocate_widths`] as a pure cell-mapping helper and
//! [`SummaryBar`] as a thin renderer that uses it + `Buffer::cell_mut`.
//!
//! Source for (shared `frontend/llr/`):
//! - `Chat detail summary bar proportional to visible segments`
//!
//! and (new `frontend/tui/llr/`):
//! - `TUI Chat detail summary bar paints via Buffer cell_mut`

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};

/// One segment painted into the summary bar.
#[derive(Debug, Clone)]
pub struct SummarySeg {
    /// Stable id (the visible-frontier node id). Used by the hover-test that
    /// brightens the segment when its tree row is focused.
    pub id: String,
    pub bytes: usize,
    pub color: Color,
    /// Optional inline label painted as a 1-character abbreviation when there
    /// is room. Currently unused but reserved for future tweaks.
    pub label: Option<String>,
}

/// The renderer. Pure stateless; called per-frame.
pub struct SummaryBar<'a> {
    pub segments: &'a [SummarySeg],
    /// Optional id of the currently-focused tree row. Used by
    /// [`SummaryBar::render_hover_indicator`] to locate the cell range to
    /// point at. The id may name a visible-frontier segment OR any ancestor
    /// of one (matching is exact-equal or prefix `"{hovered}/"`, since node
    /// ids are slash-delimited paths and a frontier's descendants of a given
    /// ancestor are contiguous in the segment list).
    pub hovered: Option<&'a str>,
}

impl<'a> SummaryBar<'a> {
    /// Paint the colored bar into the first row of `area`. Hover state does
    /// not affect this row; see [`Self::render_hover_indicator`] for the
    /// separate pointer row.
    pub fn render(&self, area: Rect, buf: &mut Buffer) {
        if area.width == 0 || area.height == 0 {
            return;
        }
        let widths = self.widths(area.width);
        let y = area.y;
        let mut x = area.x;
        for (i, seg) in self.segments.iter().enumerate() {
            let w = widths[i];
            let style = Style::default().fg(seg.color);
            for dx in 0..w {
                let cx = x + dx;
                if cx >= area.x + area.width {
                    break;
                }
                if let Some(cell) = buf.cell_mut((cx, y)) {
                    cell.set_symbol("█").set_style(style);
                }
            }
            x += w;
        }
        // Fill any trailing gap (e.g., total == 0) with dim dots.
        let dim = Style::default()
            .fg(Color::DarkGray)
            .add_modifier(Modifier::DIM);
        while x < area.x + area.width {
            if let Some(cell) = buf.cell_mut((x, y)) {
                cell.set_symbol("·").set_style(dim);
            }
            x += 1;
        }
    }

    /// Paint a yellow `▲` pointer row into the first row of `area`, spanning
    /// exactly the horizontal cell range of the segments that match the
    /// hovered id (either the segment's own id or any ancestor prefix). All
    /// non-matching cells in `area` are blanked with spaces so a previously-
    /// painted indicator never leaves stale glyphs across frames.
    pub fn render_hover_indicator(&self, area: Rect, buf: &mut Buffer) {
        if area.width == 0 || area.height == 0 {
            return;
        }
        let y = area.y;
        // 1. Blank the row first to clear any leftover glyphs.
        for x in area.x..area.x + area.width {
            if let Some(cell) = buf.cell_mut((x, y)) {
                cell.set_symbol(" ").set_style(Style::default());
            }
        }
        let Some(hov) = self.hovered else { return; };
        let widths = self.widths(area.width);
        let arrow_style = Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD);
        let prefix = format!("{hov}/");
        let mut x = area.x;
        for (i, seg) in self.segments.iter().enumerate() {
            let w = widths[i];
            let matches = seg.id == hov || seg.id.starts_with(&prefix);
            if matches {
                for dx in 0..w {
                    let cx = x + dx;
                    if cx >= area.x + area.width {
                        break;
                    }
                    if let Some(cell) = buf.cell_mut((cx, y)) {
                        cell.set_symbol("▲").set_style(arrow_style);
                    }
                }
            }
            x += w;
        }
    }

    fn widths(&self, width: u16) -> Vec<u16> {
        let total: usize = self.segments.iter().map(|s| s.bytes).sum();
        let weights: Vec<usize> = self.segments.iter().map(|s| s.bytes).collect();
        allocate_widths(total, &weights, width)
    }
}

/// Distribute `width` cells across `segs` proportional to weight, distributing
/// the rounding remainder to the largest fractional parts so the result sums
/// to exactly `width`.
///
/// Invariants:
///   * `out.len() == segs.len()`
///   * `out.iter().sum::<u16>() == width` whenever `total > 0`
///   * `total == 0` → every cell is 0 (renderer fills with dim placeholder)
pub fn allocate_widths(total: usize, segs: &[usize], width: u16) -> Vec<u16> {
    let n = segs.len();
    if n == 0 || width == 0 {
        return vec![0; n];
    }
    if total == 0 {
        return vec![0; n];
    }
    let total_f = total as f64;
    let width_f = width as f64;
    // Floor pass.
    let mut floors: Vec<u16> = Vec::with_capacity(n);
    let mut fracs: Vec<(usize, f64)> = Vec::with_capacity(n);
    let mut used: u32 = 0;
    for (i, s) in segs.iter().enumerate() {
        let raw = (*s as f64 / total_f) * width_f;
        let f = raw.floor();
        floors.push(f as u16);
        used += f as u32;
        fracs.push((i, raw - f));
    }
    let remainder = (width as u32).saturating_sub(used) as usize;
    // Award the remainder to the segments with the largest fractional parts,
    // breaking ties by original index (stable). Only segments with non-zero
    // weight participate.
    fracs.sort_by(|a, b| {
        b.1.partial_cmp(&a.1)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.0.cmp(&b.0))
    });
    for (i, _) in fracs.iter().take(remainder) {
        floors[*i] = floors[*i].saturating_add(1);
    }
    floors
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_segments_returns_empty() {
        assert!(allocate_widths(0, &[], 10).is_empty());
        assert!(allocate_widths(100, &[], 10).is_empty());
    }

    #[test]
    fn zero_total_yields_all_zero() {
        let w = allocate_widths(0, &[0, 0, 0], 30);
        assert_eq!(w, vec![0, 0, 0]);
    }

    #[test]
    fn zero_width_yields_all_zero() {
        let w = allocate_widths(10, &[1, 2, 3], 0);
        assert_eq!(w, vec![0, 0, 0]);
    }

    #[test]
    fn single_segment_takes_full_width() {
        let w = allocate_widths(10, &[10], 20);
        assert_eq!(w, vec![20]);
    }

    #[test]
    fn sum_equals_width_for_uneven_split() {
        let total = 7;
        let segs = [1, 2, 4];
        let width = 13;
        let w = allocate_widths(total, &segs, width);
        assert_eq!(w.iter().copied().map(u32::from).sum::<u32>(), width as u32);
    }

    #[test]
    fn larger_weights_get_proportionally_more_cells() {
        let w = allocate_widths(10, &[1, 9], 20);
        assert!(w[1] > w[0]);
    }

    #[test]
    fn remainder_distributed_to_largest_fractions() {
        // total=3, weights [1,1,1], width=4 → each gets 4*1/3 = 1.333. One
        // cell remainder; ties broken by index → first segment gets the extra.
        let w = allocate_widths(3, &[1, 1, 1], 4);
        assert_eq!(w.iter().copied().map(u32::from).sum::<u32>(), 4);
        assert_eq!(w[0], 2); // tied frac; index 0 wins.
    }

    #[test]
    fn render_paints_at_least_two_colors_for_two_segments() {
        let segs = vec![
            SummarySeg {
                id: "a".into(),
                bytes: 50,
                color: Color::Red,
                label: None,
            },
            SummarySeg {
                id: "b".into(),
                bytes: 50,
                color: Color::Blue,
                label: None,
            },
        ];
        let bar = SummaryBar {
            segments: &segs,
            hovered: None,
        };
        let area = Rect::new(0, 0, 10, 1);
        let mut buf = Buffer::empty(area);
        bar.render(area, &mut buf);
        let mut colors: std::collections::HashSet<Color> = std::collections::HashSet::new();
        for x in 0..10 {
            colors.insert(buf[(x, 0)].fg);
        }
        assert!(colors.len() >= 2, "expected ≥ 2 distinct colours, got {:?}", colors);
    }

    #[test]
    fn render_does_not_brighten_hovered_segment() {
        let segs = vec![SummarySeg {
            id: "a".into(),
            bytes: 1,
            color: Color::Red,
            label: None,
        }];
        let bar = SummaryBar {
            segments: &segs,
            hovered: Some("a"),
        };
        let area = Rect::new(0, 0, 5, 1);
        let mut buf = Buffer::empty(area);
        bar.render(area, &mut buf);
        for x in 0..5 {
            assert!(
                !buf[(x, 0)].modifier.contains(Modifier::REVERSED),
                "bar row should not carry REVERSED at x={x}",
            );
        }
    }

    #[test]
    fn hover_indicator_paints_yellow_arrows_under_matched_segment() {
        let segs = vec![
            SummarySeg {
                id: "a".into(),
                bytes: 50,
                color: Color::Red,
                label: None,
            },
            SummarySeg {
                id: "b".into(),
                bytes: 50,
                color: Color::Blue,
                label: None,
            },
        ];
        let bar = SummaryBar {
            segments: &segs,
            hovered: Some("b"),
        };
        let bar_area = Rect::new(0, 0, 10, 1);
        let ind_area = Rect::new(0, 1, 10, 1);
        let mut buf = Buffer::empty(Rect::new(0, 0, 10, 2));
        bar.render(bar_area, &mut buf);
        bar.render_hover_indicator(ind_area, &mut buf);

        // The matched segment ("b") occupies the right half (cols 5..10).
        for x in 0..5 {
            assert_eq!(buf[(x, 1)].symbol(), " ", "left half should be blank at x={x}");
        }
        for x in 5..10 {
            assert_eq!(buf[(x, 1)].symbol(), "▲", "right half should have arrow at x={x}");
            assert_eq!(buf[(x, 1)].fg, Color::Yellow);
            assert!(buf[(x, 1)].modifier.contains(Modifier::BOLD));
        }
    }

    #[test]
    fn hover_indicator_blanks_row_when_no_hover() {
        let segs = vec![SummarySeg {
            id: "a".into(),
            bytes: 1,
            color: Color::Red,
            label: None,
        }];
        let bar = SummaryBar {
            segments: &segs,
            hovered: None,
        };
        let area = Rect::new(0, 0, 5, 1);
        let mut buf = Buffer::empty(area);
        // Pre-fill so we can verify the renderer overwrites stale glyphs.
        for x in 0..5 {
            buf[(x, 0)].set_symbol("▲");
        }
        bar.render_hover_indicator(area, &mut buf);
        for x in 0..5 {
            assert_eq!(buf[(x, 0)].symbol(), " ", "stale glyph should be cleared at x={x}");
        }
    }

    #[test]
    fn hover_indicator_blanks_row_when_id_does_not_match() {
        let segs = vec![SummarySeg {
            id: "a".into(),
            bytes: 1,
            color: Color::Red,
            label: None,
        }];
        let bar = SummaryBar {
            segments: &segs,
            hovered: Some("zzz"),
        };
        let area = Rect::new(0, 0, 5, 1);
        let mut buf = Buffer::empty(area);
        for x in 0..5 {
            buf[(x, 0)].set_symbol("▲");
        }
        bar.render_hover_indicator(area, &mut buf);
        for x in 0..5 {
            assert_eq!(buf[(x, 0)].symbol(), " ");
        }
    }

    #[test]
    fn hover_indicator_matches_ancestor_via_slash_prefix() {
        // Two visible-frontier descendants under a common ancestor "p".
        let segs = vec![
            SummarySeg {
                id: "p/a".into(),
                bytes: 50,
                color: Color::Red,
                label: None,
            },
            SummarySeg {
                id: "p/b".into(),
                bytes: 50,
                color: Color::Blue,
                label: None,
            },
            SummarySeg {
                id: "q".into(),
                bytes: 0,
                color: Color::Green,
                label: None,
            },
        ];
        let bar = SummaryBar {
            segments: &segs,
            hovered: Some("p"),
        };
        let area = Rect::new(0, 0, 10, 1);
        let mut buf = Buffer::empty(area);
        bar.render_hover_indicator(area, &mut buf);
        // Both "p/a" and "p/b" descendants get arrows; "q" has zero bytes so
        // contributes no cells anyway. The full 10-cell row should be arrows.
        for x in 0..10 {
            assert_eq!(buf[(x, 0)].symbol(), "▲", "expected arrow at x={x}");
            assert_eq!(buf[(x, 0)].fg, Color::Yellow);
        }
    }
}
