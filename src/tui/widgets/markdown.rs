//! `markdown_to_lines` — render a Markdown string to a flat list of ratatui
//! [`Line`]s. Terminal analog of the web `react-markdown` + `remark-gfm`
//! rendering used by the `task` / `read_agent` tool-detail renderers.
//!
//! This is a pragmatic, single-pass event mapping — not a full Markdown
//! engine. Block structure (paragraphs, headings, lists, code blocks) and the
//! common inline styles (emphasis, strong, inline code, links) are mapped to
//! ratatui styling; everything else degrades to plain text.
//!
//! Event → style mapping:
//! - Heading H1..H6 → `Color::Yellow` + `Modifier::BOLD`, prefixed with the
//!   matching number of `#`.
//! - Paragraph → plain text; a blank line separates blocks.
//! - Unordered list item → `"• "` prefix; ordered → `"N. "` prefix.
//! - Fenced/indented code block → `Color::DarkGray`, one [`Line`] per source
//!   line, no inline parsing.
//! - Inline code → `Modifier::REVERSED`.
//! - Emphasis → `Modifier::ITALIC`; Strong → `Modifier::BOLD`.
//! - Link → underlined text followed by ` (url)`.
//! - Hard/soft break → new line.
//!
//! Source for (new `frontend/tui/llr/`):
//! - `TUI Markdown to lines via pulldown-cmark`
//!
//! Also source for (shared `frontend/llr/`):
//! - `Task tool renders prompt as markdown`
//! - `Read agent tool renders result as markdown`

use pulldown_cmark::{Event, HeadingLevel, Options, Parser, Tag, TagEnd};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

/// Render `md` to ratatui lines wrapped at `width` cells. `width == 0` is
/// treated as unbounded (no wrapping).
pub fn markdown_to_lines(md: &str, width: u16) -> Vec<Line<'static>> {
    let mut opts = Options::empty();
    opts.insert(Options::ENABLE_STRIKETHROUGH);
    opts.insert(Options::ENABLE_TABLES);
    let parser = Parser::new_ext(md, opts);

    let mut b = Builder::default();
    for ev in parser {
        b.event(ev);
    }
    b.finish();

    if width == 0 {
        return b.lines;
    }
    wrap_lines(b.lines, width)
}

#[derive(Default)]
struct Builder {
    lines: Vec<Line<'static>>,
    /// Spans accumulated for the current (not-yet-flushed) line.
    cur: Vec<Span<'static>>,
    /// Active inline style stack (emphasis/strong/code/link).
    style: Style,
    /// Nesting of list markers: `Some(n)` ordered counter, `None` unordered.
    list_stack: Vec<Option<u64>>,
    /// True while inside a fenced/indented code block.
    in_code_block: bool,
    /// Pending link URL to append after the link text closes.
    link_url: Option<String>,
    /// True once any block content has been emitted (controls blank-line
    /// separators between blocks).
    emitted_block: bool,
}

impl Builder {
    fn event(&mut self, ev: Event<'_>) {
        match ev {
            Event::Start(tag) => self.start(tag),
            Event::End(tag) => self.end(tag),
            Event::Text(t) => self.text(&t),
            Event::Code(c) => {
                let s = self.style.add_modifier(Modifier::REVERSED);
                self.cur.push(Span::styled(c.to_string(), s));
            }
            Event::SoftBreak | Event::HardBreak => self.flush_line(),
            Event::Rule => {
                self.block_gap();
                self.lines
                    .push(Line::from(Span::styled("───", Style::default().fg(Color::DarkGray))));
                self.emitted_block = true;
            }
            _ => {}
        }
    }

    fn start(&mut self, tag: Tag<'_>) {
        match tag {
            Tag::Heading { level, .. } => {
                self.block_gap();
                let hashes = "#".repeat(heading_n(level));
                self.cur.push(Span::styled(
                    format!("{hashes} "),
                    Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
                ));
                self.style = Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD);
            }
            Tag::Paragraph => self.block_gap(),
            Tag::List(start) => {
                self.list_stack.push(start);
            }
            Tag::Item => {
                self.flush_line();
                let depth = self.list_stack.len().saturating_sub(1);
                let indent = "  ".repeat(depth);
                let marker = match self.list_stack.last_mut() {
                    Some(Some(n)) => {
                        let m = format!("{n}. ");
                        *n += 1;
                        m
                    }
                    _ => "• ".to_string(),
                };
                self.cur.push(Span::raw(format!("{indent}{marker}")));
            }
            Tag::CodeBlock(_) => {
                self.block_gap();
                self.in_code_block = true;
            }
            Tag::Emphasis => self.style = self.style.add_modifier(Modifier::ITALIC),
            Tag::Strong => self.style = self.style.add_modifier(Modifier::BOLD),
            Tag::Strikethrough => self.style = self.style.add_modifier(Modifier::CROSSED_OUT),
            Tag::Link { dest_url, .. } => {
                self.style = self.style.add_modifier(Modifier::UNDERLINED);
                self.link_url = Some(dest_url.to_string());
            }
            _ => {}
        }
    }

    fn end(&mut self, tag: TagEnd) {
        match tag {
            TagEnd::Heading(_) => {
                self.flush_line();
                self.style = Style::default();
                self.emitted_block = true;
            }
            TagEnd::Paragraph => {
                self.flush_line();
                self.emitted_block = true;
            }
            TagEnd::List(_) => {
                self.flush_line();
                self.list_stack.pop();
                self.emitted_block = true;
            }
            TagEnd::Item => self.flush_line(),
            TagEnd::CodeBlock => {
                self.in_code_block = false;
                self.emitted_block = true;
            }
            TagEnd::Emphasis => self.style = self.style.remove_modifier(Modifier::ITALIC),
            TagEnd::Strong => self.style = self.style.remove_modifier(Modifier::BOLD),
            TagEnd::Strikethrough => self.style = self.style.remove_modifier(Modifier::CROSSED_OUT),
            TagEnd::Link => {
                self.style = self.style.remove_modifier(Modifier::UNDERLINED);
                if let Some(url) = self.link_url.take() {
                    self.cur.push(Span::styled(
                        format!(" ({url})"),
                        Style::default().fg(Color::DarkGray),
                    ));
                }
            }
            _ => {}
        }
    }

    fn text(&mut self, t: &str) {
        if self.in_code_block {
            for (i, line) in t.split('\n').enumerate() {
                if i > 0 {
                    self.flush_line();
                }
                if !line.is_empty() {
                    self.cur.push(Span::styled(
                        line.to_string(),
                        Style::default().fg(Color::DarkGray),
                    ));
                }
            }
            return;
        }
        self.cur.push(Span::styled(t.to_string(), self.style));
    }

    /// Ensure a single blank line separates the previous block from the next.
    fn block_gap(&mut self) {
        self.flush_line();
        if self.emitted_block && self.lines.last().map(|l| !is_blank(l)).unwrap_or(false) {
            self.lines.push(Line::from(""));
        }
    }

    fn flush_line(&mut self) {
        if self.cur.is_empty() {
            return;
        }
        let spans = std::mem::take(&mut self.cur);
        self.lines.push(Line::from(spans));
    }

    fn finish(&mut self) {
        self.flush_line();
    }
}

fn is_blank(line: &Line<'_>) -> bool {
    line.spans.iter().all(|s| s.content.trim().is_empty())
}

fn heading_n(level: HeadingLevel) -> usize {
    match level {
        HeadingLevel::H1 => 1,
        HeadingLevel::H2 => 2,
        HeadingLevel::H3 => 3,
        HeadingLevel::H4 => 4,
        HeadingLevel::H5 => 5,
        HeadingLevel::H6 => 6,
    }
}

/// Wrap each rendered line to `width` cells, preserving per-span styles. A
/// naive char-boundary wrap (no word breaking) — adequate for narrow columns.
fn wrap_lines(lines: Vec<Line<'static>>, width: u16) -> Vec<Line<'static>> {
    let w = width.max(1) as usize;
    let mut out: Vec<Line<'static>> = Vec::with_capacity(lines.len());
    for line in lines {
        let total: usize = line.spans.iter().map(|s| s.content.chars().count()).sum();
        if total <= w {
            out.push(line);
            continue;
        }
        let mut cur: Vec<Span<'static>> = Vec::new();
        let mut col = 0usize;
        for span in line.spans {
            let style = span.style;
            for ch in span.content.chars() {
                if col >= w {
                    out.push(Line::from(std::mem::take(&mut cur)));
                    col = 0;
                }
                cur.push(Span::styled(ch.to_string(), style));
                col += 1;
            }
        }
        if !cur.is_empty() {
            out.push(Line::from(cur));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text_of(lines: &[Line<'_>]) -> String {
        lines
            .iter()
            .map(|l| l.spans.iter().map(|s| s.content.as_ref()).collect::<String>())
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn heading_rendered_with_hashes_and_style() {
        let lines = markdown_to_lines("# Title\n", 0);
        let joined = text_of(&lines);
        assert!(joined.contains("# Title"), "got: {joined:?}");
        // Heading span carries bold + yellow.
        let styled = lines
            .iter()
            .flat_map(|l| l.spans.iter())
            .any(|s| s.style.add_modifier.contains(Modifier::BOLD) && s.style.fg == Some(Color::Yellow));
        assert!(styled);
    }

    #[test]
    fn unordered_list_marker() {
        let lines = markdown_to_lines("- one\n- two\n", 0);
        let joined = text_of(&lines);
        assert!(joined.contains("• one"), "got: {joined:?}");
        assert!(joined.contains("• two"), "got: {joined:?}");
    }

    #[test]
    fn ordered_list_marker() {
        let lines = markdown_to_lines("1. first\n2. second\n", 0);
        let joined = text_of(&lines);
        assert!(joined.contains("1. first"), "got: {joined:?}");
        assert!(joined.contains("2. second"), "got: {joined:?}");
    }

    #[test]
    fn inline_code_reversed() {
        let lines = markdown_to_lines("use `cargo build` now\n", 0);
        let styled = lines
            .iter()
            .flat_map(|l| l.spans.iter())
            .any(|s| s.content.contains("cargo build") && s.style.add_modifier.contains(Modifier::REVERSED));
        assert!(styled);
    }

    #[test]
    fn fenced_code_block_dim() {
        let md = "```\nlet x = 1;\n```\n";
        let lines = markdown_to_lines(md, 0);
        let styled = lines
            .iter()
            .flat_map(|l| l.spans.iter())
            .any(|s| s.content.contains("let x = 1;") && s.style.fg == Some(Color::DarkGray));
        assert!(styled, "got: {:?}", text_of(&lines));
    }

    #[test]
    fn link_renders_text_and_url() {
        let lines = markdown_to_lines("see [docs](https://example.com)\n", 0);
        let joined = text_of(&lines);
        assert!(joined.contains("docs"), "got: {joined:?}");
        assert!(joined.contains("https://example.com"), "got: {joined:?}");
    }

    #[test]
    fn paragraph_break_emits_blank_line() {
        let lines = markdown_to_lines("para one\n\npara two\n", 0);
        let joined = text_of(&lines);
        assert!(joined.contains("para one"));
        assert!(joined.contains("para two"));
        // A blank line separates the two paragraphs.
        assert!(joined.contains("\n\n"), "got: {joined:?}");
    }
}
