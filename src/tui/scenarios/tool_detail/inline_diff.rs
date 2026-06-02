//! Inline line-level diff for the `edit` tool's `old_str` → `new_str` change.
//!
//! Mirrors the web `InlineDiff` component (`web/src/scenarios/ToolDetail.tsx`),
//! minus the word-level intra-line emphasis — that requires per-row paired
//! masks at paint time and is deliberately deferred to a follow-up.
//!
//! Produces one [`DiffRow`] per displayed line in the unified column order
//! Myers' diff emits (each hunk: removed lines, then added lines, with equal
//! context in between). The renderer turns the row stream into:
//!
//! * a body string with a 2-char left marker per line (`- `, `+ `, `  `),
//! * a per-byte [`Style`] vector painting Rem lines red and Add lines green,
//! * a per-line gutter slice carrying the row's primary line number
//!   (new-side for Add/Eq, old-side for Rem),
//!
//! all of which feed directly into the existing
//! `BodyCtx::search_block(key, text, base_styles, gutter)` pipeline so the
//! diff participates in per-block `/` search and external-query highlighting
//! exactly like every other body block.
//!
//! Replaces the +/- chip's old-vs-new total-line counts (which over-counted
//! a tiny edit as "+N -N" instead of "+1 -1") with the real diff-row counts
//! by exposing [`count_changes`].

use ratatui::style::{Color, Style};
use similar::{ChangeTag, TextDiff};

/// One row of the unified inline-diff display.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffRow {
    pub kind: RowKind,
    /// Old-side (1-based) line number, present on Eq and Rem rows.
    pub old_no: Option<u32>,
    /// New-side (1-based) line number, present on Eq and Add rows.
    pub new_no: Option<u32>,
    pub text: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowKind {
    Eq,
    Add,
    Rem,
}

/// Build the inline-diff row stream from a removed-text → added-text pair.
/// Line endings are normalised (CRLF → LF) before diffing so a CRLF-vs-LF
/// difference doesn't drown the diff in spurious noise.
pub fn build(old: &str, new: &str) -> Vec<DiffRow> {
    let old = old.replace("\r\n", "\n").replace('\r', "\n");
    let new = new.replace("\r\n", "\n").replace('\r', "\n");
    let diff = TextDiff::from_lines(&old, &new);
    let mut out: Vec<DiffRow> = Vec::new();
    let mut old_no: u32 = 1;
    let mut new_no: u32 = 1;
    for change in diff.iter_all_changes() {
        // similar's per-line `value()` keeps the trailing `\n`. Strip it for
        // display — the renderer joins rows with its own `\n` and the
        // gutter/marker layout doesn't want an interior newline.
        let raw = change.value();
        let text = raw.strip_suffix('\n').unwrap_or(raw).to_string();
        match change.tag() {
            ChangeTag::Equal => {
                out.push(DiffRow {
                    kind: RowKind::Eq,
                    old_no: Some(old_no),
                    new_no: Some(new_no),
                    text,
                });
                old_no += 1;
                new_no += 1;
            }
            ChangeTag::Delete => {
                out.push(DiffRow {
                    kind: RowKind::Rem,
                    old_no: Some(old_no),
                    new_no: None,
                    text,
                });
                old_no += 1;
            }
            ChangeTag::Insert => {
                out.push(DiffRow {
                    kind: RowKind::Add,
                    old_no: None,
                    new_no: Some(new_no),
                    text,
                });
                new_no += 1;
            }
        }
    }
    out
}

/// Count of `(added, removed)` rows. Used by the Spans row's +/- chip so
/// the chip reflects real diff arithmetic rather than the total line counts
/// of `old_str` / `new_str` (which over-counts a one-line touch on a
/// many-line block as `+N -N`).
pub fn count_changes(rows: &[DiffRow]) -> (u32, u32) {
    let mut added = 0u32;
    let mut removed = 0u32;
    for r in rows {
        match r.kind {
            RowKind::Add => added += 1,
            RowKind::Rem => removed += 1,
            RowKind::Eq => {}
        }
    }
    (added, removed)
}

/// Render the row stream to the `(text, base_styles, gutter)` triple the
/// `BodyCtx::search_block` pipeline expects.
///
/// Each row is rendered as `"<marker><space><line>"`:
/// * `"- "` for Rem (red foreground),
/// * `"+ "` for Add (green foreground),
/// * `"  "` for Eq (default).
///
/// The gutter carries the primary line number: new-side for Add/Eq,
/// old-side for Rem. The marker bytes stay inside the body so wrap, search,
/// and external-query highlighting all see the diff as one flat
/// text block — no special-case painter needed.
pub fn render(rows: &[DiffRow]) -> (String, Vec<Style>, Vec<Option<u64>>) {
    let mut text = String::new();
    let mut styles: Vec<Style> = Vec::new();
    let mut gutter: Vec<Option<u64>> = Vec::with_capacity(rows.len());

    let rem_style = Style::default().fg(Color::Red);
    let add_style = Style::default().fg(Color::Green);
    let eq_style = Style::default();

    for (i, row) in rows.iter().enumerate() {
        if i > 0 {
            text.push('\n');
            // The `\n` separator carries default style; the
            // `styles.len() == text.len()` invariant matters because the
            // overlay path indexes byte-for-byte.
            styles.push(eq_style);
        }
        let (marker, style) = match row.kind {
            RowKind::Add => ("+ ", add_style),
            RowKind::Rem => ("- ", rem_style),
            RowKind::Eq => ("  ", eq_style),
        };
        text.push_str(marker);
        styles.extend(std::iter::repeat_n(style, marker.len()));
        text.push_str(&row.text);
        styles.extend(std::iter::repeat_n(style, row.text.len()));

        let primary = match row.kind {
            RowKind::Add | RowKind::Eq => row.new_no,
            RowKind::Rem => row.old_no,
        };
        gutter.push(primary.map(|n| n as u64));
    }
    debug_assert_eq!(styles.len(), text.len());
    debug_assert_eq!(gutter.len(), rows.len());
    (text, styles, gutter)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_single_line_change_yields_one_rem_and_one_add() {
        let rows = build("a\n", "b\n");
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].kind, RowKind::Rem);
        assert_eq!(rows[0].text, "a");
        assert_eq!(rows[0].old_no, Some(1));
        assert_eq!(rows[0].new_no, None);
        assert_eq!(rows[1].kind, RowKind::Add);
        assert_eq!(rows[1].text, "b");
        assert_eq!(rows[1].old_no, None);
        assert_eq!(rows[1].new_no, Some(1));
    }

    #[test]
    fn build_unchanged_lines_show_as_eq_with_both_line_numbers() {
        let rows = build("a\nb\nc\n", "a\nx\nc\n");
        // a (eq), b (rem), x (add), c (eq)
        assert_eq!(rows.len(), 4);
        assert_eq!(rows[0].kind, RowKind::Eq);
        assert_eq!(rows[0].old_no, Some(1));
        assert_eq!(rows[0].new_no, Some(1));
        assert_eq!(rows[1].kind, RowKind::Rem);
        assert_eq!(rows[2].kind, RowKind::Add);
        assert_eq!(rows[3].kind, RowKind::Eq);
        assert_eq!(rows[3].old_no, Some(3));
        assert_eq!(rows[3].new_no, Some(3));
    }

    #[test]
    fn count_changes_only_counts_diff_rows() {
        let rows = build("a\nb\nc\n", "a\nx\ny\nc\n");
        // a eq, b rem, x add, y add, c eq → (2, 1)
        assert_eq!(count_changes(&rows), (2, 1));
    }

    #[test]
    fn count_changes_one_char_change_in_five_line_block() {
        // The bug it replaces: old chip path returned (5, 5) for any
        // 5-line edit. Real answer: changed exactly one line.
        let old = "fn a() {\n    let x = 1;\n    println!(\"x\");\n    return;\n}\n";
        let new = "fn a() {\n    let x = 2;\n    println!(\"x\");\n    return;\n}\n";
        let rows = build(old, new);
        let (added, removed) = count_changes(&rows);
        assert_eq!((added, removed), (1, 1), "single-line change must report (1, 1)");
    }

    #[test]
    fn render_invariants_text_styles_gutter_match_rows() {
        let rows = build("a\nb\n", "x\nb\n");
        let (text, styles, gutter) = render(&rows);
        assert_eq!(styles.len(), text.len());
        assert_eq!(gutter.len(), rows.len());
        // Text spans 3 rows: "- a", "+ x", "  b" joined by \n.
        assert_eq!(text, "- a\n+ x\n  b");
        // Gutter: rem → old_no 1; add → new_no 1; eq → new_no 2.
        assert_eq!(gutter, vec![Some(1), Some(1), Some(2)]);
        // Rem row's marker byte is red, Add row's is green, Eq is default.
        assert_eq!(styles[0].fg, Some(Color::Red));
        let nl1 = text.find('\n').unwrap();
        assert_eq!(styles[nl1 + 1].fg, Some(Color::Green));
        let nl2 = text[nl1 + 1..].find('\n').unwrap() + nl1 + 1;
        assert_eq!(styles[nl2 + 1].fg, None);
    }

    #[test]
    fn crlf_normalises_so_no_spurious_diff() {
        let rows = build("a\r\nb\r\n", "a\nb\n");
        // After CRLF normalisation both sides are "a\nb\n" → all Eq.
        assert!(rows.iter().all(|r| r.kind == RowKind::Eq));
    }
}
