---
type: LLR
tags:
  - req/llr
  - tui
  - domain/tool-detail
---
The edit-tool inline diff renderer (`tool_detail::inline_diff::build`) MUST produce one `DiffRow` per displayed line using `similar::TextDiff::from_lines`, normalising CRLF→LF (and bare CR→LF) on both `old_str` and `new_str` before diffing. Each row MUST carry its old-side and/or new-side 1-based line numbers (both set on `Eq`, only `old_no` on `Rem`, only `new_no` on `Add`) and the text stripped of any per-row trailing `
`. The renderer MUST emit row text into the body with a 2-char left marker (`- `, `+ `, `  `), per-byte `Style` painting `Rem` rows red (`Color::Red`) and `Add` rows green (`Color::Green`), and a per-line gutter slice (`new_no` for Eq/Add, `old_no` for Rem). The rendered block MUST flow through the existing `BodyCtx::search_block(key, text, base_styles, gutter)` pipeline so per-block `/` search and external-query highlighting apply identically to every other body block.

## Rationale
Bridges the web `InlineDiff` component into the TUI tool-detail column without breaking the per-block search contract. Word-level intra-line emphasis is deliberately deferred.

## Derived from
- [[Tool detail inline diff for edit tool]]
