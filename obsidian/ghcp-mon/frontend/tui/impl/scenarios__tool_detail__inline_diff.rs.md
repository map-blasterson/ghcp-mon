---
type: impl
source: src/tui/scenarios/tool_detail/inline_diff.rs
lang: rust
tags:
  - impl/original
  - impl/rust
---
Inline line-level diff for the `edit` tool's `old_str` → `new_str`. `build(old, new)` normalises CRLF→LF, runs `similar::TextDiff::from_lines`, and emits one `DiffRow { kind: Eq|Add|Rem, old_no, new_no, text }` per displayed line with trailing `
` stripped. `count_changes(rows)` returns `(adds, rems)` for the +/- chip counting fix. Consumers wire the rows into the `BodyCtx::search_block` pipeline with a 2-char marker, per-byte red/green styles, and the per-line gutter slice.

## Source For
- [[TUI Tool detail edit inline diff renders unified rows]]
- [[TUI Tool detail edit diff chip counts diff rows not block totals]]
- [[Tool detail inline diff for edit tool]]
