//! Searchable text block — a reusable ratatui widget that renders a body of
//! text with an inline, keyboard-driven case-insensitive search affordance.
//!
//! This is the terminal port of the web `TextBlock` component. The
//! cursor-following `?/` hover glyph and the mouse click/right-click gestures
//! have no faithful terminal analog, so they are substituted with keyboard
//! gestures and a static `[?]` corner hint (see the TUI-only LLRs below).
//!
//! Source for (shared `frontend/llr/`):
//! - `TextBlock cursor-following search hint icon`
//! - `TextBlock click activates search input`
//! - `TextBlock highlights case-insensitive matches`
//! - `TextBlock click cycles matches shift for previous`
//! - `TextBlock Enter cycles Escape exits`
//! - `TextBlock exits search on column-body leave or right-click`
//! - `TextBlock current match scrolls into view`
//! - `TextBlock truncatable controlled by open prop`
//! - `TextBlock search header shows match counter`
//! - `TextBlock accepts external search query prop`
//! - `TextBlock external query suppresses interactive exit gestures`
//!
//! and (new `frontend/tui/llr/`):
//! - `TUI SearchableTextBlock static question-mark hint replaces cursor-follow glyph`
//! - `TUI SearchableTextBlock wrap and locate`
//! - `TUI SearchableTextBlock scroll into view on cycle`
//! - `TUI SearchableTextBlock truncate with ellipsis on last visible row`
//! - `TUI SearchableTextBlock external query suppresses Esc and focus-loss`
//! - `TUI SearchableTextBlock focus-lost is the column-mouseleave analog`
//! - `TUI SearchableTextBlock match counter sticky header`
//!
//! ## Dispatch
//! Per the §"Key-dispatch policy" the host routes keys to [`SearchableTextBlock::handle_key`]
//! when the block has focus (the **widget** precedence layer). While
//! `phase == Active` and the search input is editable, character keys and
//! `Backspace` are consumed verbatim by the input (the **text-input** layer,
//! highest precedence) — that precedence is honored *inside* `handle_key`.

use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Span;

use super::search_input::{SearchInput, SearchInputView};

// ---- Glyph / wrap model ----------------------------------------------------

/// A single rendered character together with its byte offset in the original
/// (un-wrapped) `text`. Tracking the byte offset is what lets match offsets be
/// computed against the **wrapped** glyph stream while still pointing back into
/// the source string.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Glyph {
    /// The character.
    pub ch: char,
    /// Byte offset of this character in the original `text`.
    pub byte_offset: usize,
}

/// One wrapped visual row: a run of [`Glyph`]s that fit within the wrap width.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WrappedRow {
    /// Glyphs on this row, in left-to-right order.
    pub glyphs: Vec<Glyph>,
    /// Byte offset in `text` where this row begins.
    pub row_start_byte: usize,
}

/// A located match, expressed in wrapped-stream coordinates.
///
/// `(row, col)` is the start cell; `(row_end, col_end)` is the cell just past
/// the last matched glyph, so a match that straddles a wrap boundary is
/// representable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MatchPos {
    /// Byte offset of the match start in `text`.
    pub byte_offset: usize,
    /// Byte length of the matched substring.
    pub byte_len: usize,
    /// Wrapped row of the first matched glyph.
    pub row: u16,
    /// Wrapped column of the first matched glyph.
    pub col: u16,
    /// Wrapped row of the cell just past the match.
    pub row_end: u16,
    /// Wrapped column of the cell just past the match.
    pub col_end: u16,
}

// ---- State -----------------------------------------------------------------

/// The three lifecycle phases of the block.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SearchPhase {
    /// Not focused, not searching.
    #[default]
    Idle,
    /// Focused but search not active — shows the `[?]` corner hint.
    Icon,
    /// Search active — header, highlights, and input bar are rendered.
    Active,
}

/// Per-block state owned by the parent scenario. Cheap to clone.
#[derive(Debug, Clone, Default)]
pub struct SearchableTextBlockState {
    /// Current lifecycle phase.
    pub phase: SearchPhase,
    /// Current search query (the chars typed into the input, or the external
    /// query when externally driven).
    pub query: String,
    /// Index of the current cycled match.
    pub match_index: usize,
    /// Number of matches found (recomputed on every render).
    pub match_count: usize,
    /// Cycle delta queued by Enter/Shift+Enter before the next render has
    /// recomputed `match_count`.
    pending_cycle: isize,
    /// Top wrapped-row index currently visible.
    pub scroll_top: u16,
    /// Single-line edit/cursor state for the search input. Owned here (not
    /// borrowed) so a parent can keep one `SearchableTextBlockState` per block
    /// without threading a second struct.
    input: SearchInput,
    /// Tracks whether the *current* Active phase was entered by an external
    /// query, so render() can detect the `Some(non-empty) -> None/""`
    /// transition and exit without needing the caller to remember the previous
    /// prop value.
    external_active: bool,
}

impl SearchableTextBlockState {
    /// Record the latest match count and apply any queued cycle request from a
    /// key event that arrived before render had located matches.
    pub fn set_match_count(&mut self, match_count: usize) {
        self.match_count = match_count;
        if self.match_count == 0 {
            self.match_index = 0;
            self.pending_cycle = 0;
            return;
        }
        if self.match_index >= self.match_count {
            self.match_index = self.match_count - 1;
        }
        if self.pending_cycle != 0 {
            let n = self.match_count as isize;
            self.match_index =
                (self.match_index as isize + self.pending_cycle).rem_euclid(n) as usize;
            self.pending_cycle = 0;
        }
    }

    /// Cycle to the next or previous match. If render has not computed matches
    /// for the current query yet, queue the cycle for `set_match_count`.
    pub fn cycle_match(&mut self, reverse: bool) {
        let delta = if reverse { -1 } else { 1 };
        if self.match_count > 0 {
            let n = self.match_count as isize;
            self.match_index = (self.match_index as isize + delta).rem_euclid(n) as usize;
        } else if !self.query.is_empty() {
            self.pending_cycle += delta;
        }
    }

    fn clear_pending_cycle(&mut self) {
        self.pending_cycle = 0;
    }
}

// ---- Widget ----------------------------------------------------------------

/// Stateless renderer constructed per-frame.
pub struct SearchableTextBlock<'a> {
    /// The body text to render and search.
    pub text: &'a str,
    /// When `truncatable && !open`, only `truncate_rows` rows are rendered with
    /// a `…` indicator on the last visible row.
    pub truncatable: bool,
    /// Parent-controlled expand flag (only meaningful when `truncatable`).
    pub open: bool,
    /// Number of wrapped rows to show when truncated.
    pub truncate_rows: u16,
    /// When `Some(non-empty)`, drives the lifecycle programmatically: forces
    /// Active with this query, replaces the typed query verbatim, and
    /// suppresses interactive exit (Esc / focus-loss). `None`/`Some("")` ends
    /// the externally-driven Active phase.
    pub external_query: Option<&'a str>,
    /// Whether this block currently has TUI focus. Drives Idle↔Icon.
    pub focused: bool,
}

impl<'a> SearchableTextBlock<'a> {
    /// Render the block into `area`, mutating `state` (phase reconciliation,
    /// match count, clamp, and scroll-into-view all happen here).
    pub fn render(self, area: Rect, buf: &mut Buffer, state: &mut SearchableTextBlockState) {
        if area.width == 0 || area.height == 0 {
            return;
        }

        Self::reconcile_phase(state, self.external_query, self.focused);

        let active = state.phase == SearchPhase::Active;

        // Keep the embedded input view in sync with `query` (covers callers
        // that set `query` directly, and the externally-driven case).
        if active && state.input.text() != state.query {
            state.input.set_text(state.query.clone());
        }

        let header_h: u16 = if active { 1 } else { 0 };
        let input_h: u16 = if active { 1 } else { 0 };

        let body_x = area.x;
        let inner_w = area.width;
        let body_y = area.y + header_h;
        let body_h = area.height.saturating_sub(header_h + input_h);

        // Visible body height: truncated blocks cap at `truncate_rows`.
        let plot_h = if self.truncatable && !self.open {
            self.truncate_rows.min(body_h)
        } else {
            body_h
        };

        // Wrap + locate against the wrapped stream.
        let rows = wrap_text(self.text, inner_w);
        let total_rows = rows.len() as u16;
        let matches = if active {
            locate_matches(self.text, &state.query, inner_w)
        } else {
            Vec::new()
        };

        state.set_match_count(matches.len());

        // Clamp the current match index and scroll it into view.
        if state.match_count > 0 {
            if state.match_index >= state.match_count {
                state.match_index = state.match_count - 1;
            }
            if plot_h > 0 {
                let cur_row = matches[state.match_index].row;
                if cur_row < state.scroll_top {
                    state.scroll_top = cur_row;
                } else if cur_row >= state.scroll_top + plot_h {
                    state.scroll_top = cur_row.saturating_sub(plot_h.saturating_sub(1));
                }
            }
        } else {
            state.match_index = 0;
            if !self.truncatable {
                state.scroll_top = 0;
            }
        }

        // ---- Body ----
        let cur_range = matches
            .get(state.match_index)
            .map(|m| (m.byte_offset, m.byte_offset + m.byte_len));

        if plot_h > 0 {
            for vis in 0..plot_h {
                let r = state.scroll_top + vis;
                let Some(row) = rows.get(r as usize) else {
                    break;
                };
                let screen_y = body_y + vis;
                for (ci, g) in row.glyphs.iter().enumerate() {
                    let x = body_x + ci as u16;
                    if x >= body_x + inner_w {
                        break;
                    }
                    let is_current = cur_range
                        .map(|(s, e)| g.byte_offset >= s && g.byte_offset < e)
                        .unwrap_or(false);
                    let in_match = matches
                        .iter()
                        .any(|m| g.byte_offset >= m.byte_offset && g.byte_offset < m.byte_offset + m.byte_len);
                    let style = if is_current {
                        Style::default()
                            .bg(Color::Rgb(255, 200, 0))
                            .fg(Color::Black)
                            .add_modifier(Modifier::BOLD)
                    } else if in_match {
                        Style::default().bg(Color::Yellow).fg(Color::Black)
                    } else {
                        Style::default()
                    };
                    let cell = &mut buf[(x, screen_y)];
                    cell.set_char(g.ch);
                    cell.set_style(style);
                }
            }

            // Truncation ellipsis on the last visible row when rows are hidden.
            if self.truncatable
                && !self.open
                && (state.scroll_top + plot_h) < total_rows
                && inner_w > 0
            {
                let ex = body_x + inner_w - 1;
                let ey = body_y + plot_h - 1;
                let cell = &mut buf[(ex, ey)];
                cell.set_char('…');
                cell.set_style(Style::default().fg(Color::DarkGray).add_modifier(Modifier::DIM));
            }
        }

        // ---- Sticky header (Active) ----
        if active {
            let hstyle = Style::default().fg(Color::DarkGray).add_modifier(Modifier::DIM);
            let left = if !state.query.is_empty() && state.match_count > 0 {
                format!("{} of {} matches", state.match_index + 1, state.match_count)
            } else {
                "0 matches".to_string()
            };
            let right = "(shift)+CR prev/next  Esc exit";
            buf.set_span(area.x, area.y, &Span::styled(left, hstyle), inner_w);
            let rw = right.chars().count() as u16;
            if rw < inner_w {
                buf.set_span(
                    area.x + inner_w - rw,
                    area.y,
                    &Span::styled(right, hstyle),
                    rw,
                );
            }
        }

        // ---- Icon hint (Icon phase) ----
        if state.phase == SearchPhase::Icon && inner_w >= 3 {
            let hint = Span::styled(
                "[?]",
                Style::default().fg(Color::DarkGray).add_modifier(Modifier::DIM),
            );
            buf.set_span(area.x + inner_w - 3, area.y, &hint, 3);
        }

        // ---- Input bar (Active) ----
        if active {
            let iy = area.y + area.height - 1;
            let view = SearchInputView {
                state: &state.input,
                focused: true,
                prompt: "/ ",
            };
            // SearchInputView is a Widget; render directly over the bottom row.
            use ratatui::widgets::Widget;
            view.render(Rect::new(area.x, iy, inner_w, 1), buf);
        }
    }

    /// Reconcile the phase against `external_query` and `focused`. Idempotent
    /// per frame.
    fn reconcile_phase(
        state: &mut SearchableTextBlockState,
        external_query: Option<&str>,
        focused: bool,
    ) {
        let ext = external_query.filter(|s| !s.is_empty());
        match ext {
            Some(q) => {
                state.phase = SearchPhase::Active;
                if state.query != q {
                    state.query = q.to_string();
                    state.input.set_text(q);
                    state.match_index = 0;
                    state.clear_pending_cycle();
                }
                state.external_active = true;
            }
            None => {
                if state.external_active {
                    // The external lifecycle just ended — exit Active.
                    reset_search(state);
                    state.phase = if focused {
                        SearchPhase::Icon
                    } else {
                        SearchPhase::Idle
                    };
                    state.external_active = false;
                }
            }
        }

        // Focus-driven Idle <-> Icon (never disturbs Active).
        match state.phase {
            SearchPhase::Idle if focused => state.phase = SearchPhase::Icon,
            SearchPhase::Icon if !focused => state.phase = SearchPhase::Idle,
            _ => {}
        }
    }

    /// Process a keyboard event in the context of this widget. Returns `true`
    /// if the event was consumed.
    ///
    /// Honors the text-input precedence internally: when `phase == Active`
    /// (and not externally driven) character keys + `Backspace` are consumed
    /// verbatim by the embedded input.
    pub fn handle_key(
        key: KeyEvent,
        state: &mut SearchableTextBlockState,
        external_query: Option<&str>,
    ) -> bool {
        let ext_active = external_query.map(|s| !s.is_empty()).unwrap_or(false);

        match state.phase {
            SearchPhase::Idle | SearchPhase::Icon => {
                if matches!(key.code, KeyCode::Char('/')) && !ext_active {
                    state.phase = SearchPhase::Active;
                    state.query.clear();
                    state.input.clear();
                    state.match_index = 0;
                    state.match_count = 0;
                    state.clear_pending_cycle();
                    true
                } else {
                    false
                }
            }
            SearchPhase::Active => match key.code {
                KeyCode::Esc => {
                    if ext_active {
                        // Suppressed — external owner controls the lifecycle.
                        // Do not consume; let the host fall through.
                        return false;
                    }
                    reset_search(state);
                    state.phase = SearchPhase::Idle;
                    true
                }
                KeyCode::Enter => {
                    state.cycle_match(key.modifiers.contains(KeyModifiers::SHIFT));
                    true
                }
                _ => {
                    if ext_active {
                        // External query is shown verbatim; ignore edit keys.
                        return false;
                    }
                    let consumed = state.input.handle_key(key);
                    if consumed {
                        let new_q = state.input.text().to_string();
                        if new_q != state.query {
                            state.query = new_q;
                            state.match_index = 0;
                            state.clear_pending_cycle();
                        }
                    }
                    consumed
                }
            },
        }
    }

    /// Public phase-reconcile hook for hosts that render a *custom* body (e.g.
    /// a syntect [`crate::tui::widgets::code_block::CodeBlock`]) but still want
    /// the external-query / focus lifecycle managed by this widget. After this
    /// call `state.phase` and `state.query` are up to date, so the host can
    /// branch on `state.phase == SearchPhase::Active`.
    pub fn reconcile(
        state: &mut SearchableTextBlockState,
        external_query: Option<&str>,
        focused: bool,
    ) {
        Self::reconcile_phase(state, external_query, focused);
    }

    /// Called by the host when the parent column loses focus. No-op when an
    /// external query is set (the external owner controls the lifecycle);
    /// otherwise resets to Idle and clears search state.
    pub fn on_focus_lost(state: &mut SearchableTextBlockState, external_query: Option<&str>) {
        if external_query.map(|s| !s.is_empty()).unwrap_or(false) {
            return;
        }
        reset_search(state);
        state.phase = SearchPhase::Idle;
    }
}

/// Reset all transient search state (query, matches, scroll, input) without
/// touching `phase` — callers set the phase explicitly.
fn reset_search(state: &mut SearchableTextBlockState) {
    state.query.clear();
    state.input.clear();
    state.match_index = 0;
    state.match_count = 0;
    state.clear_pending_cycle();
    state.scroll_top = 0;
}

// ---- Pure helpers ----------------------------------------------------------

/// Wrap `text` into a stream of glyph rows of at most `width` cells, preserving
/// the original byte offset of every glyph.
///
/// Wrapping rules:
/// - Each character occupies one cell (no East-Asian-width handling; documented
///   limitation — wide glyphs still count as a single cell).
/// - `\n` ends the current row; a blank line yields an empty [`WrappedRow`].
/// - `\r\n` is normalized: the `\r` is dropped and the `\n` breaks the row. A
///   lone `\r` (not followed by `\n`) is kept as an ordinary glyph.
/// - A trailing `\n` does **not** synthesize an empty trailing row.
/// - `width == 0` is treated as `1` to avoid an infinite loop.
pub fn wrap_text(text: &str, width: u16) -> Vec<WrappedRow> {
    if text.is_empty() {
        return Vec::new();
    }
    let w = width.max(1) as usize;

    // Split into logical lines on `\n` first (each carrying its absolute byte
    // start), then soft-wrap each logical line independently. Splitting first
    // means a hard newline immediately after a width-flush does not synthesize
    // a spurious empty row.
    let mut segments: Vec<(usize, &str)> = Vec::new();
    let mut line_start = 0usize;
    for (i, ch) in text.char_indices() {
        if ch == '\n' {
            // Strip a CRLF `\r` immediately preceding the LF.
            let mut end = i;
            if end > line_start && text.as_bytes()[end - 1] == b'\r' {
                end -= 1;
            }
            segments.push((line_start, &text[line_start..end]));
            line_start = i + ch.len_utf8();
        }
    }
    segments.push((line_start, &text[line_start..]));
    // A trailing newline (last segment is the empty tail) does not synthesize
    // an empty trailing row.
    if text.ends_with('\n') {
        segments.pop();
    }

    let mut rows: Vec<WrappedRow> = Vec::new();
    for (seg_start, seg) in segments {
        let mut glyphs: Vec<Glyph> = Vec::new();
        let mut row_start_byte = seg_start;
        for (li, ch) in seg.char_indices() {
            let abs = seg_start + li;
            if glyphs.is_empty() {
                row_start_byte = abs;
            }
            glyphs.push(Glyph {
                ch,
                byte_offset: abs,
            });
            if glyphs.len() >= w {
                rows.push(WrappedRow {
                    glyphs: std::mem::take(&mut glyphs),
                    row_start_byte,
                });
                row_start_byte = abs + ch.len_utf8();
            }
        }
        if !glyphs.is_empty() {
            rows.push(WrappedRow {
                glyphs,
                row_start_byte,
            });
        } else if seg.is_empty() {
            // A blank logical line yields a single empty visual row.
            rows.push(WrappedRow {
                glyphs: Vec::new(),
                row_start_byte: seg_start,
            });
        }
    }
    rows
}

/// Locate every case-insensitive, **non-overlapping** match of `query` in
/// `text` and map each to its wrapped-stream coordinate.
///
/// - An empty `query`, or a `query` longer than `text`, returns an empty list.
/// - Matches do not overlap: after a hit the scan resumes past the match end,
///   so `"aa"` in `"aaa"` yields a single match.
/// - Case-insensitivity uses Unicode simple lowercasing per character; byte
///   offsets always point into the *original* `text`.
/// - Matches are assumed not to contain a hard `\n` (queries are single
///   tokens); a query spanning a *soft* wrap boundary is fully supported.
pub fn locate_matches(text: &str, query: &str, width: u16) -> Vec<MatchPos> {
    let qc: Vec<char> = query.chars().collect();
    if qc.is_empty() {
        return Vec::new();
    }
    let tc: Vec<(usize, char)> = text.char_indices().collect();
    if qc.len() > tc.len() {
        return Vec::new();
    }

    let rows = wrap_text(text, width);
    // Flatten to (row, col) and index by byte offset.
    let mut flat: Vec<(u16, u16)> = Vec::new();
    let mut by_byte: std::collections::HashMap<usize, usize> = std::collections::HashMap::new();
    for (r, row) in rows.iter().enumerate() {
        for (c, g) in row.glyphs.iter().enumerate() {
            by_byte.insert(g.byte_offset, flat.len());
            flat.push((r as u16, c as u16));
        }
    }

    let mut out: Vec<MatchPos> = Vec::new();
    let mut i = 0usize;
    while i + qc.len() <= tc.len() {
        let hit = (0..qc.len()).all(|j| ci_eq(tc[i + j].1, qc[j]));
        if hit {
            let byte_offset = tc[i].0;
            let last = i + qc.len() - 1;
            let byte_end = tc[last].0 + tc[last].1.len_utf8();
            let byte_len = byte_end - byte_offset;

            // Coordinates come from the flat glyph index.
            if let Some(&si) = by_byte.get(&byte_offset) {
                let (row, col) = flat[si];
                let ei = si + qc.len();
                let (row_end, col_end) = if ei < flat.len() {
                    flat[ei]
                } else {
                    let (lr, lc) = flat[si + qc.len() - 1];
                    (lr, lc + 1)
                };
                out.push(MatchPos {
                    byte_offset,
                    byte_len,
                    row,
                    col,
                    row_end,
                    col_end,
                });
            }
            i += qc.len(); // non-overlapping
        } else {
            i += 1;
        }
    }
    out
}

/// Case-insensitive single-character comparison (Unicode simple lowercase).
fn ci_eq(a: char, b: char) -> bool {
    a == b || a.to_lowercase().eq(b.to_lowercase())
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::backend::TestBackend;
    use ratatui::crossterm::event::{KeyEventKind, KeyEventState};
    use ratatui::{Terminal, layout::Rect};

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent {
            code,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        }
    }

    fn key_mod(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
        KeyEvent {
            code,
            modifiers,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        }
    }

    // ---- wrap_text -------------------------------------------------------

    #[test]
    fn wrap_empty_string() {
        assert!(wrap_text("", 10).is_empty());
    }

    #[test]
    fn wrap_single_line_under_width() {
        let rows = wrap_text("hello", 10);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].glyphs.len(), 5);
        assert_eq!(rows[0].row_start_byte, 0);
    }

    #[test]
    fn wrap_hard_newlines_preserve_blank_line() {
        let rows = wrap_text("a\n\nb", 10);
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0].glyphs.len(), 1);
        assert!(rows[1].glyphs.is_empty());
        assert_eq!(rows[2].glyphs[0].ch, 'b');
        assert_eq!(rows[2].glyphs[0].byte_offset, 3);
    }

    #[test]
    fn wrap_width_one() {
        let rows = wrap_text("abc", 1);
        assert_eq!(rows.len(), 3);
        for (i, r) in rows.iter().enumerate() {
            assert_eq!(r.glyphs.len(), 1);
            assert_eq!(r.glyphs[0].byte_offset, i);
        }
    }

    #[test]
    fn wrap_zero_width_treated_as_one() {
        let rows = wrap_text("ab", 0);
        assert_eq!(rows.len(), 2);
    }

    #[test]
    fn wrap_non_ascii_byte_offsets() {
        // "héllo" — é is 2 bytes.
        let rows = wrap_text("héllo", 10);
        assert_eq!(rows.len(), 1);
        let g = &rows[0].glyphs;
        assert_eq!(g[0].byte_offset, 0); // h
        assert_eq!(g[1].byte_offset, 1); // é
        assert_eq!(g[2].byte_offset, 3); // l (é took 2 bytes)
        assert_eq!(g[1].ch, 'é');
    }

    #[test]
    fn wrap_trailing_newline_no_empty_row() {
        let rows = wrap_text("abc\n", 10);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].glyphs.len(), 3);
    }

    #[test]
    fn wrap_crlf_tolerated() {
        // \r is dropped, \n breaks.
        let rows = wrap_text("a\r\nb", 10);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].glyphs.len(), 1);
        assert_eq!(rows[0].glyphs[0].ch, 'a');
        assert_eq!(rows[1].glyphs[0].ch, 'b');
    }

    #[test]
    fn wrap_lone_cr_is_a_glyph() {
        let rows = wrap_text("a\rb", 10);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].glyphs.len(), 3);
        assert_eq!(rows[0].glyphs[1].ch, '\r');
    }

    #[test]
    fn wrap_soft_wrap_at_width() {
        let rows = wrap_text("abcdef", 3);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].glyphs.len(), 3);
        assert_eq!(rows[1].glyphs.len(), 3);
        assert_eq!(rows[1].row_start_byte, 3);
    }

    // ---- locate_matches --------------------------------------------------

    #[test]
    fn locate_no_matches() {
        assert!(locate_matches("hello world", "xyz", 40).is_empty());
    }

    #[test]
    fn locate_one_match() {
        let m = locate_matches("hello world", "world", 40);
        assert_eq!(m.len(), 1);
        assert_eq!(m[0].byte_offset, 6);
        assert_eq!(m[0].byte_len, 5);
        assert_eq!(m[0].row, 0);
        assert_eq!(m[0].col, 6);
    }

    #[test]
    fn locate_case_insensitive() {
        let m = locate_matches("Hello HELLO hello", "hello", 40);
        assert_eq!(m.len(), 3);
    }

    #[test]
    fn locate_overlapping_are_non_overlapping() {
        // "aaa" with "aa" -> a single match (scan resumes past match end).
        let m = locate_matches("aaa", "aa", 40);
        assert_eq!(m.len(), 1);
        assert_eq!(m[0].byte_offset, 0);
    }

    #[test]
    fn locate_multi_row_match() {
        // width 3 -> "abc" | "def"; query "cd" straddles the wrap boundary.
        let m = locate_matches("abcdef", "cd", 3);
        assert_eq!(m.len(), 1);
        assert_eq!(m[0].row, 0);
        assert_eq!(m[0].col, 2);
        assert_eq!(m[0].row_end, 1);
        assert_eq!(m[0].col_end, 1);
    }

    #[test]
    fn locate_empty_query_returns_empty() {
        assert!(locate_matches("hello", "", 40).is_empty());
    }

    #[test]
    fn locate_query_longer_than_text() {
        assert!(locate_matches("hi", "hello", 40).is_empty());
    }

    #[test]
    fn locate_match_at_end_row_end_past_text() {
        let m = locate_matches("abc", "bc", 40);
        assert_eq!(m.len(), 1);
        assert_eq!(m[0].col, 1);
        assert_eq!(m[0].col_end, 3); // just past 'c'
    }

    // ---- state transitions ----------------------------------------------

    fn render_active(state: &mut SearchableTextBlockState, text: &str, w: u16, h: u16) -> Buffer {
        let backend = TestBackend::new(w, h);
        let mut term = Terminal::new(backend).unwrap();
        term.draw(|f| {
            let widget = SearchableTextBlock {
                text,
                truncatable: false,
                open: true,
                truncate_rows: 0,
                external_query: None,
                focused: true,
            };
            widget.render(Rect::new(0, 0, w, h), f.buffer_mut(), state);
        })
        .unwrap();
        term.backend().buffer().clone()
    }

    #[test]
    fn idle_to_icon_on_focus() {
        let mut state = SearchableTextBlockState::default();
        let w = SearchableTextBlock {
            text: "hello",
            truncatable: false,
            open: true,
            truncate_rows: 0,
            external_query: None,
            focused: true,
        };
        let backend = TestBackend::new(20, 3);
        let mut term = Terminal::new(backend).unwrap();
        term.draw(|f| w.render(Rect::new(0, 0, 20, 3), f.buffer_mut(), &mut state))
            .unwrap();
        assert_eq!(state.phase, SearchPhase::Icon);
    }

    #[test]
    fn icon_to_active_on_slash() {
        let mut state = SearchableTextBlockState {
            phase: SearchPhase::Icon,
            ..Default::default()
        };
        assert!(SearchableTextBlock::handle_key(
            key(KeyCode::Char('/')),
            &mut state,
            None
        ));
        assert_eq!(state.phase, SearchPhase::Active);
    }

    #[test]
    fn active_to_idle_on_esc() {
        let mut state = SearchableTextBlockState {
            phase: SearchPhase::Active,
            query: "x".into(),
            ..Default::default()
        };
        assert!(SearchableTextBlock::handle_key(key(KeyCode::Esc), &mut state, None));
        assert_eq!(state.phase, SearchPhase::Idle);
        assert!(state.query.is_empty());
    }

    #[test]
    fn external_query_enters_active() {
        let mut state = SearchableTextBlockState::default();
        render_with_external(&mut state, "hello world", Some("world"));
        assert_eq!(state.phase, SearchPhase::Active);
        assert_eq!(state.query, "world");
        assert_eq!(state.match_count, 1);
    }

    #[test]
    fn external_query_to_none_exits_active() {
        let mut state = SearchableTextBlockState::default();
        render_with_external(&mut state, "hello world", Some("world"));
        assert_eq!(state.phase, SearchPhase::Active);
        // External cleared -> exits.
        render_with_external(&mut state, "hello world", None);
        assert_ne!(state.phase, SearchPhase::Active);
        assert!(state.query.is_empty());
    }

    #[test]
    fn esc_noop_when_external_set() {
        let mut state = SearchableTextBlockState {
            phase: SearchPhase::Active,
            query: "world".into(),
            external_active: true,
            ..Default::default()
        };
        SearchableTextBlock::handle_key(key(KeyCode::Esc), &mut state, Some("world"));
        assert_eq!(state.phase, SearchPhase::Active);
        assert_eq!(state.query, "world");
    }

    #[test]
    fn on_focus_lost_clears_when_no_external() {
        let mut state = SearchableTextBlockState {
            phase: SearchPhase::Active,
            query: "x".into(),
            ..Default::default()
        };
        SearchableTextBlock::on_focus_lost(&mut state, None);
        assert_eq!(state.phase, SearchPhase::Idle);
        assert!(state.query.is_empty());
    }

    #[test]
    fn on_focus_lost_noop_when_external() {
        let mut state = SearchableTextBlockState {
            phase: SearchPhase::Active,
            query: "world".into(),
            external_active: true,
            ..Default::default()
        };
        SearchableTextBlock::on_focus_lost(&mut state, Some("world"));
        assert_eq!(state.phase, SearchPhase::Active);
        assert_eq!(state.query, "world");
    }

    // ---- cycle -----------------------------------------------------------

    fn active_state_with_matches(text: &str, query: &str, w: u16) -> SearchableTextBlockState {
        let mut state = SearchableTextBlockState {
            phase: SearchPhase::Active,
            query: query.into(),
            ..Default::default()
        };
        // Render once to populate match_count.
        render_active(&mut state, text, w, 6);
        state
    }

    #[test]
    fn enter_wraps_forward() {
        let mut state = active_state_with_matches("a a a", "a", 40);
        assert_eq!(state.match_count, 3);
        state.match_index = 2;
        SearchableTextBlock::handle_key(key(KeyCode::Enter), &mut state, None);
        assert_eq!(state.match_index, 0);
    }

    #[test]
    fn shift_enter_wraps_backward() {
        let mut state = active_state_with_matches("a a a", "a", 40);
        state.match_index = 0;
        SearchableTextBlock::handle_key(
            key_mod(KeyCode::Enter, KeyModifiers::SHIFT),
            &mut state,
            None,
        );
        assert_eq!(state.match_index, 2);
    }

    #[test]
    fn enter_and_shift_enter_advance_and_decrement() {
        let mut state = active_state_with_matches("a a a", "a", 40);
        assert_eq!(state.match_index, 0);
        SearchableTextBlock::handle_key(key(KeyCode::Enter), &mut state, None);
        assert_eq!(state.match_index, 1);
        SearchableTextBlock::handle_key(
            key_mod(KeyCode::Enter, KeyModifiers::SHIFT),
            &mut state,
            None,
        );
        assert_eq!(state.match_index, 0);
    }

    #[test]
    fn enter_before_render_cycles_after_matches_are_located() {
        let mut state = SearchableTextBlockState {
            phase: SearchPhase::Active,
            query: "a".into(),
            ..Default::default()
        };
        SearchableTextBlock::handle_key(key(KeyCode::Enter), &mut state, None);
        assert_eq!(state.match_index, 0);
        assert_eq!(state.match_count, 0);
        render_active(&mut state, "a a a", 40, 6);
        assert_eq!(state.match_count, 3);
        assert_eq!(state.match_index, 1);
    }

    #[test]
    fn cycle_noop_when_no_matches() {
        let mut state = SearchableTextBlockState {
            phase: SearchPhase::Active,
            query: "zzz".into(),
            match_count: 0,
            ..Default::default()
        };
        SearchableTextBlock::handle_key(key(KeyCode::Enter), &mut state, None);
        assert_eq!(state.match_index, 0);
    }

    // ---- scroll-into-view ------------------------------------------------

    #[test]
    fn scroll_into_view_below_window() {
        // 6 single-char rows (width 1), plot height 2. Match on row 4 -> scroll.
        let text = "a\nb\nc\nd\ne\nf";
        let mut state = SearchableTextBlockState {
            phase: SearchPhase::Active,
            query: "e".into(),
            ..Default::default()
        };
        // height = 2 body rows + header(1) + input(1) = 4 total.
        render_active(&mut state, text, 1, 4);
        // 'e' is on wrapped row 4; plot_h = 2 -> scroll_top = 4 - (2-1) = 3.
        assert_eq!(state.match_count, 1);
        assert_eq!(state.scroll_top, 3);
    }

    #[test]
    fn scroll_into_view_above_window() {
        let text = "a\nb\nc\nd\ne\nf";
        let mut state = SearchableTextBlockState {
            phase: SearchPhase::Active,
            query: "a".into(),
            scroll_top: 4,
            ..Default::default()
        };
        render_active(&mut state, text, 1, 4);
        // 'a' is row 0, above scroll_top -> scroll_top = 0.
        assert_eq!(state.scroll_top, 0);
    }

    #[test]
    fn scroll_into_view_inside_window_unchanged() {
        let text = "a\nb\nc\nd\ne\nf";
        let mut state = SearchableTextBlockState {
            phase: SearchPhase::Active,
            query: "b".into(),
            scroll_top: 0,
            ..Default::default()
        };
        render_active(&mut state, text, 1, 4);
        // 'b' is row 1, within [0,2) -> unchanged.
        assert_eq!(state.scroll_top, 0);
    }

    // ---- truncation ------------------------------------------------------

    fn render_truncated(
        state: &mut SearchableTextBlockState,
        text: &str,
        truncate_rows: u16,
        w: u16,
        h: u16,
    ) -> Buffer {
        let backend = TestBackend::new(w, h);
        let mut term = Terminal::new(backend).unwrap();
        term.draw(|f| {
            let widget = SearchableTextBlock {
                text,
                truncatable: true,
                open: false,
                truncate_rows,
                external_query: None,
                focused: false,
            };
            widget.render(Rect::new(0, 0, w, h), f.buffer_mut(), state);
        })
        .unwrap();
        term.backend().buffer().clone()
    }

    #[test]
    fn truncation_hides_rows_and_shows_ellipsis() {
        // width 1 -> 5 rows; truncate to 2.
        let mut state = SearchableTextBlockState::default();
        let buf = render_truncated(&mut state, "abcde", 2, 1, 5);
        // Row 0 = 'a', row 1 = ellipsis (last visible row last cell).
        assert_eq!(buf[(0, 0)].symbol(), "a");
        assert_eq!(buf[(0, 1)].symbol(), "…");
        // Row 2 ('c') must NOT be rendered.
        assert_ne!(buf[(0, 2)].symbol(), "c");
    }

    #[test]
    fn truncation_counts_hidden_matches() {
        // Externally-driven search so it stays active while truncated.
        let mut state = SearchableTextBlockState::default();
        let backend = TestBackend::new(1, 8);
        let mut term = Terminal::new(backend).unwrap();
        term.draw(|f| {
            let widget = SearchableTextBlock {
                text: "x\nx\nx\nx",
                truncatable: true,
                open: false,
                truncate_rows: 1,
                external_query: Some("x"),
                focused: true,
            };
            widget.render(Rect::new(0, 0, 1, 8), f.buffer_mut(), &mut state);
        })
        .unwrap();
        // All 4 'x' matches counted even though only 1 row is visible.
        assert_eq!(state.match_count, 4);
    }

    // ---- rendering -------------------------------------------------------

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

    fn render_with_external(
        state: &mut SearchableTextBlockState,
        text: &str,
        external: Option<&str>,
    ) -> Buffer {
        let backend = TestBackend::new(40, 6);
        let mut term = Terminal::new(backend).unwrap();
        term.draw(|f| {
            let widget = SearchableTextBlock {
                text,
                truncatable: false,
                open: true,
                truncate_rows: 0,
                external_query: external,
                focused: true,
            };
            widget.render(Rect::new(0, 0, 40, 6), f.buffer_mut(), state);
        })
        .unwrap();
        term.backend().buffer().clone()
    }

    #[test]
    fn active_shows_header_and_input() {
        let mut state = SearchableTextBlockState {
            phase: SearchPhase::Active,
            query: "world".into(),
            ..Default::default()
        };
        let buf = render_active(&mut state, "hello world", 60, 6);
        let text = buf_to_string(&buf);
        assert!(text.contains("1 of 1 matches"), "header missing in:\n{text}");
        assert!(text.contains("Esc exit"), "hint missing in:\n{text}");
        assert!(text.contains("/ world"), "input row missing in:\n{text}");
    }

    #[test]
    fn icon_shows_question_mark_top_right() {
        let mut state = SearchableTextBlockState::default();
        let w = SearchableTextBlock {
            text: "hello",
            truncatable: false,
            open: true,
            truncate_rows: 0,
            external_query: None,
            focused: true,
        };
        let backend = TestBackend::new(20, 3);
        let mut term = Terminal::new(backend).unwrap();
        term.draw(|f| w.render(Rect::new(0, 0, 20, 3), f.buffer_mut(), &mut state))
            .unwrap();
        let buf = term.backend().buffer().clone();
        // top-right 3 cells.
        assert_eq!(buf[(17, 0)].symbol(), "[");
        assert_eq!(buf[(18, 0)].symbol(), "?");
        assert_eq!(buf[(19, 0)].symbol(), "]");
    }

    #[test]
    fn idle_shows_neither_header_nor_hint() {
        let mut state = SearchableTextBlockState::default();
        let w = SearchableTextBlock {
            text: "hello",
            truncatable: false,
            open: true,
            truncate_rows: 0,
            external_query: None,
            focused: false,
        };
        let backend = TestBackend::new(20, 3);
        let mut term = Terminal::new(backend).unwrap();
        term.draw(|f| w.render(Rect::new(0, 0, 20, 3), f.buffer_mut(), &mut state))
            .unwrap();
        let buf = term.backend().buffer().clone();
        let s = buf_to_string(&buf);
        assert_eq!(state.phase, SearchPhase::Idle);
        assert!(!s.contains("matches"));
        assert!(!s.contains("[?]"));
    }

    #[test]
    fn match_highlight_visible() {
        let mut state = SearchableTextBlockState {
            phase: SearchPhase::Active,
            query: "world".into(),
            ..Default::default()
        };
        let buf = render_active(&mut state, "hello world", 40, 6);
        // body row is y = 1 (header at 0). 'w' of world starts at col 6.
        let cell = &buf[(6, 1)];
        assert_eq!(cell.symbol(), "w");
        // The current (and only) match is the brighter highlight.
        assert_eq!(cell.style().bg, Some(Color::Rgb(255, 200, 0)));
    }

    #[test]
    fn non_current_match_is_plain_yellow() {
        let mut state = SearchableTextBlockState {
            phase: SearchPhase::Active,
            query: "x".into(),
            ..Default::default()
        };
        // Two matches; current is index 0.
        let buf = render_active(&mut state, "x y x", 40, 6);
        assert_eq!(state.match_count, 2);
        // First x at col 0 (current -> bright).
        assert_eq!(buf[(0, 1)].style().bg, Some(Color::Rgb(255, 200, 0)));
        // Second x at col 4 (non-current -> plain yellow).
        assert_eq!(buf[(4, 1)].style().bg, Some(Color::Yellow));
    }
}
