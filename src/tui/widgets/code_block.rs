//! `CodeBlock` — syntect-backed syntax highlighting widget (the terminal
//! analog of the web `CodeBlock` / Prism component).
//!
//! The default syntax + theme sets are loaded once and cached behind a
//! [`OnceLock`]. The public [`syntect_byte_styles`] helper exposes a per-byte
//! ratatui [`Style`] vector so the tool-detail scenario can paint syntect base
//! colors *underneath* search-match highlights in a single render pass.
//!
//! Source for (shared `frontend/llr/`):
//! - `Code block highlights via Prism with extension map`
//!
//! Also source for (new `frontend/tui/llr/`):
//! - `TUI CodeBlock syntect highlight rendering`

use std::sync::OnceLock;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use syntect::easy::HighlightLines;
use syntect::highlighting::{FontStyle, Theme, ThemeSet};
use syntect::parsing::SyntaxSet;

use super::lang_from_path::lang_from_path;

/// Cached default syntax set (Sublime `.sublime-syntax` definitions baked into
/// syntect via `default-fancy`).
fn syntax_set() -> &'static SyntaxSet {
    static SS: OnceLock<SyntaxSet> = OnceLock::new();
    SS.get_or_init(SyntaxSet::load_defaults_newlines)
}

/// Cached default dark theme (`base16-ocean.dark`).
fn theme() -> &'static Theme {
    static TH: OnceLock<Theme> = OnceLock::new();
    TH.get_or_init(|| {
        let ts = ThemeSet::load_defaults();
        ts.themes
            .get("base16-ocean.dark")
            .or_else(|| ts.themes.values().next())
            .cloned()
            .expect("syntect ships at least one default theme")
    })
}

/// Convert a syntect highlight [`syntect::highlighting::Style`] to a ratatui
/// [`Style`]. Only the foreground color and font modifiers are carried over;
/// the theme background is intentionally dropped so the terminal background and
/// any search-match highlight remain visible.
fn to_ratatui_style(s: syntect::highlighting::Style) -> Style {
    let mut out = Style::default().fg(Color::Rgb(s.foreground.r, s.foreground.g, s.foreground.b));
    if s.font_style.intersects(FontStyle::BOLD) {
        out = out.add_modifier(Modifier::BOLD);
    }
    if s.font_style.intersects(FontStyle::ITALIC) {
        out = out.add_modifier(Modifier::ITALIC);
    }
    if s.font_style.intersects(FontStyle::UNDERLINE) {
        out = out.add_modifier(Modifier::UNDERLINED);
    }
    out
}

/// Highlight `text` for language slug `lang` and return a per-byte ratatui
/// [`Style`] vector (`out.len() == text.len()`). Returns `None` when `lang` is
/// `None`, syntect has no matching syntax, or highlighting fails (logged via
/// `tracing::warn!`). Callers fall back to plain rendering on `None`.
pub fn syntect_byte_styles(text: &str, lang: Option<&str>) -> Option<Vec<Style>> {
    let lang = lang?;
    let ss = syntax_set();
    let syntax = ss.find_syntax_by_token(lang)?;
    let mut hl = HighlightLines::new(syntax, theme());

    let mut styles: Vec<Style> = Vec::with_capacity(text.len());
    // `highlight_line` expects newline-terminated lines (the `_newlines`
    // syntax set). `split_inclusive('\n')` keeps the trailing newline on each
    // segment so byte accounting stays exact.
    for line in text.split_inclusive('\n') {
        match hl.highlight_line(line, ss) {
            Ok(ranges) => {
                for (sty, piece) in ranges {
                    let rstyle = to_ratatui_style(sty);
                    for _ in 0..piece.len() {
                        styles.push(rstyle);
                    }
                }
            }
            Err(e) => {
                tracing::warn!(lang, error = %e, "syntect highlight_line failed; rendering plain");
                return None;
            }
        }
    }
    // Pad/truncate defensively so the invariant `len == text.len()` holds even
    // if syntect's byte accounting ever diverges.
    styles.resize(text.len(), Style::default());
    Some(styles)
}

/// Stateless syntect-highlighting renderer. Wraps at the area width and paints
/// from the top (no internal scroll). The tool-detail scenario uses
/// [`syntect_byte_styles`] directly for composed search rendering; this widget
/// is the reusable standalone form.
pub struct CodeBlock<'a> {
    /// The source text.
    pub text: &'a str,
    /// Language slug (e.g. `"rust"`), or `None` for plain rendering.
    pub language: Option<&'a str>,
}

impl<'a> CodeBlock<'a> {
    /// Convenience constructor that resolves the language from a file path.
    pub fn from_path(text: &'a str, path: &str) -> Self {
        Self {
            text,
            language: lang_from_path(path),
        }
    }

    /// Render the (wrapped) highlighted text into `area`.
    pub fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width == 0 || area.height == 0 {
            return;
        }
        let styles = syntect_byte_styles(self.text, self.language);
        let rows = super::searchable_text_block::wrap_text(self.text, area.width);
        for (ri, row) in rows.iter().enumerate() {
            if ri as u16 >= area.height {
                break;
            }
            let y = area.y + ri as u16;
            for (ci, g) in row.glyphs.iter().enumerate() {
                let x = area.x + ci as u16;
                if x >= area.x + area.width {
                    break;
                }
                let style = styles
                    .as_ref()
                    .and_then(|s| s.get(g.byte_offset).copied())
                    .unwrap_or_default();
                let cell = &mut buf[(x, y)];
                cell.set_char(g.ch);
                cell.set_style(style);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::buffer::Buffer;
    use ratatui::layout::Rect;

    #[test]
    fn byte_styles_length_matches_text_for_known_lang() {
        let text = "fn main() {}\n";
        let styles = syntect_byte_styles(text, Some("rust")).expect("rust resolves");
        assert_eq!(styles.len(), text.len());
    }

    #[test]
    fn byte_styles_none_for_unknown_lang() {
        assert!(syntect_byte_styles("x", None).is_none());
        // A slug syntect has no syntax for falls back to plain.
        assert!(syntect_byte_styles("x", Some("definitely-not-a-language")).is_none());
    }

    #[test]
    fn render_paints_styled_cells_for_rust() {
        let area = Rect::new(0, 0, 30, 4);
        let mut buf = Buffer::empty(area);
        CodeBlock {
            text: "fn main() { let x = 1; }",
            language: Some("rust"),
        }
        .render(area, &mut buf);
        // At least one cell carries a non-default foreground (syntect color).
        let any_styled = (0..area.width)
            .any(|x| buf[(x, 0)].fg != Color::Reset && buf[(x, 0)].symbol() != " ");
        assert!(any_styled, "expected at least one syntect-colored cell");
    }

    #[test]
    fn render_plain_when_no_language() {
        let area = Rect::new(0, 0, 20, 2);
        let mut buf = Buffer::empty(area);
        CodeBlock {
            text: "hello world",
            language: None,
        }
        .render(area, &mut buf);
        assert_eq!(buf[(0, 0)].symbol(), "h");
    }
}
