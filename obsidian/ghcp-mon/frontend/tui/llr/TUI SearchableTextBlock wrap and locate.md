---
type: LLR
tags:
  - req/llr
  - tui
  - domain/text-block
---
`SearchableTextBlock` MUST compute match offsets against the **wrapped** glyph stream, not the raw source string. `wrap_text(text, width)` returns `Vec<WrappedRow>`, where each `WrappedRow { glyphs: Vec<Glyph { ch, byte_offset }>, row_start_byte }` preserves the original byte offset of every glyph. `locate_matches(text, query, width)` scans the source for case-insensitive, non-overlapping occurrences of `query` and maps each to a `MatchPos { byte_offset, byte_len, row, col, row_end, col_end }` in wrapped coordinates; `(row_end, col_end)` is the cell just past the last matched glyph, so a match that straddles a soft-wrap boundary is fully representable across two rows. Highlighting iterates visible wrapped rows and paints any glyph whose `byte_offset` falls inside a match range. Wrapping rules: each char is one cell (no East-Asian width handling); `
` ends a logical line; `\r
` drops the `\r`; a trailing `
` does not synthesize an empty row; a blank logical line yields one empty visual row; `width == 0` is treated as `1`.

## Rationale
Match coordinates expressed against the wrapped stream are what make `scroll_into_view` work after line wrap — the current match's `row` is directly comparable to `scroll_top`. Computing offsets against the raw string would desynchronize from the rendered layout the moment any line wraps.

## Derived from
- [[TextBlock highlights case-insensitive matches]]
- [[Terminal Rendering Constraints]]
