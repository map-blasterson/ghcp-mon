//! Per-row paint for the Spans column's session-span-tree.
//!
//! Extracted from `crate::tui::app::App::draw_spans` so the row's cell
//! layout (depth indent, collapse glyph, kind badge, placeholder dots,
//! name truncation, chips, description label, report-intent title) is a
//! single composable [`Widget`] rather than ~150 lines of inline `x +=`
//! arithmetic.
//!
//! The widget is **purely a painter**: all per-node data (chips,
//! description, report-intent title, dim/highlight flags) is computed by
//! the caller and passed in. This keeps the renderer free of cache,
//! workspace, or scenario-state references and makes the row layout
//! testable in isolation.
//!
//! Implements the cell ordering specified by the
//! `TUI Spans tree row layout in cells` LLR.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Span;
use ratatui::widgets::Widget;

use crate::tui::model::SpanNode;
use crate::tui::widgets::kind_badge::{KindBadge, kind_label};
use crate::tui::widgets::rolling_dots;

fn display_name(node: &SpanNode) -> String {
    let raw = node.name.trim();
    if let Some(tool_name) = node.projected_tool_name() {
        if raw == tool_name {
            return String::new();
        }
        if let Some((head, tail)) = raw.rsplit_once(" - ") {
            if tail.trim() == tool_name {
                let head = head.trim();
                return if looks_like_model_name(head) {
                    String::new()
                } else {
                    head.to_string()
                };
            }
        }
    }
    node.name.clone()
}

fn looks_like_model_name(s: &str) -> bool {
    let s = s.to_ascii_lowercase();
    s.starts_with("gpt")
        || s.starts_with("claude")
        || s.starts_with("gemini")
        || s.starts_with("llama")
        || s.starts_with("mistral")
        || s.starts_with("o1")
        || s.starts_with("o3")
        || s.starts_with("o4")
}

/// One row of the Spans session-span-tree. `area` MUST be one row tall;
/// rows wider than the body width truncate the name with `…` per the LLR.
pub struct SpansTreeRow<'a> {
    pub node: &'a SpanNode,
    pub depth: usize,
    pub focused: bool,
    pub collapsed: bool,
    /// `Some(color)` when this row is a server-side search hit; the kind
    /// badge / glyph / name get a row-wide highlight background. `None`
    /// disables the highlight.
    pub row_bg: Option<Color>,
    /// Apply `Modifier::DIM` to the name span. Set when this row is a
    /// search miss OR a kind-filter miss.
    pub row_dim: bool,
    pub chips: &'a [(String, Color)],
    pub description: Option<&'a str>,
    pub report_title: Option<&'a str>,
    /// Animation counter (frames since startup); drives the placeholder
    /// rolling-dots glyph cycle.
    pub anim_tick: u64,
}

impl Widget for SpansTreeRow<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.height == 0 || area.width == 0 {
            return;
        }
        let row_y = area.y;
        let right_edge = area.x + area.width;
        let mut x = area.x + (self.depth as u16).saturating_mul(2);

        // (1) Collapse glyph (1 cell).
        let glyph = if self.node.children.is_empty() {
            " "
        } else if self.collapsed {
            "▸"
        } else {
            "▾"
        };
        let glyph_style = if self.focused {
            Style::default().bg(Color::Cyan).fg(Color::Black)
        } else if let Some(bg) = self.row_bg {
            Style::default()
                .bg(bg)
                .fg(Color::Black)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::White)
        };
        if x < right_edge {
            buf.set_span(x, row_y, &Span::styled(glyph, glyph_style), 1);
        }
        x += 2;

        // (2) Kind badge. Tool rows use the hash-coloured tool-name chip
        // instead, so skip the generic kind label there.
        if !self.node.is_tool_row() {
            let label = kind_label(self.node.kind_class);
            let badge_w = (label.chars().count() as u16 + 2).min(10);
            if x + badge_w < right_edge {
                let badge = KindBadge::new(self.node.kind_class)
                    .with_seed(self.node.name.clone());
                badge.render(Rect::new(x, row_y, badge_w, 1), buf);
                x += badge_w + 1;
            }
        }

        // (3) Placeholder rolling dots (3 cells).
        if self.node.ingestion_state == "placeholder" && x + 3 < right_edge {
            let dots = rolling_dots::frame(self.anim_tick);
            buf.set_span(
                x,
                row_y,
                &Span::styled(dots.to_string(), Style::default().fg(Color::Yellow)),
                3,
            );
            x += 4;
        }

        // (4) Name — truncated to leave room for chips + description + title.
        let chip_reserve: usize = self
            .chips
            .iter()
            .map(|(s, _)| s.chars().count() + 3)
            .sum::<usize>()
            .min(40);
        let desc_reserve = self
            .description
            .map(|s| s.chars().count() + 1)
            .unwrap_or(0);
        let title_reserve = self
            .report_title
            .map(|s| s.chars().count() + 2)
            .unwrap_or(0);
        let total_avail = right_edge.saturating_sub(x) as usize;
        let name_budget =
            total_avail.saturating_sub(chip_reserve + desc_reserve + title_reserve);
        let mut name = display_name(self.node);
        if name.chars().count() > name_budget {
            name = name
                .chars()
                .take(name_budget.saturating_sub(1))
                .collect::<String>();
            if !name.is_empty() {
                name.push('…');
            }
        }
        let mut name_style = if self.focused {
            Style::default()
                .bg(Color::Cyan)
                .fg(Color::Black)
                .add_modifier(Modifier::BOLD)
        } else if self.row_bg.is_some() {
            Style::default()
                .bg(Color::Yellow)
                .fg(Color::Black)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::White)
        };
        if self.row_dim {
            name_style = name_style
                .fg(Color::DarkGray)
                .add_modifier(Modifier::DIM);
        }
        if x < right_edge {
            let name_w = (name.chars().count() as u16)
                .min(right_edge.saturating_sub(x));
            buf.set_span(x, row_y, &Span::styled(name, name_style), name_w);
            x += name_w;
        }

        // (5) Chips — each is `" text "` styled as `bg-color + black-fg + bold`.
        for (text, color) in self.chips {
            if x + 1 >= right_edge {
                break;
            }
            x += 1;
            let chip_text = format!(" {text} ");
            let chip_w = (chip_text.chars().count() as u16)
                .min(right_edge.saturating_sub(x));
            let chip_style = Style::default()
                .bg(*color)
                .fg(Color::Black)
                .add_modifier(Modifier::BOLD);
            buf.set_span(x, row_y, &Span::styled(chip_text, chip_style), chip_w);
            x += chip_w;
        }

        // (6) Tool description label — plain white, no chip styling, per
        // `Spans tool description inline label`.
        if let Some(desc) = self.description {
            if x + 1 < right_edge {
                x += 1;
                let w = (desc.chars().count() as u16)
                    .min(right_edge.saturating_sub(x));
                buf.set_span(
                    x,
                    row_y,
                    &Span::styled(
                        desc.to_string(),
                        Style::default().fg(Color::White),
                    ),
                    w,
                );
                x += w;
            }
        }

        // (7) Report-intent title — plain white, no chip styling, per
        // `Report intent title shows on parent row`.
        if let Some(title) = self.report_title {
            if x + 1 < right_edge {
                x += 1;
                let w = (title.chars().count() as u16)
                    .min(right_edge.saturating_sub(x));
                buf.set_span(
                    x,
                    row_y,
                    &Span::styled(
                        title.to_string(),
                        Style::default().fg(Color::White),
                    ),
                    w,
                );
            }
        }
    }
}
