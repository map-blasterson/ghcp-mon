---
type: impl
source: src/tui/scenarios/tool_detail/render_edit.rs
lang: rust
tags:
  - impl/generated
  - impl/rust
---
Edit/write renderer: path kv + syntect-highlighted `old_str`/`new_str` (or `content`) blocks keyed by the file extension, remaining args under `other` as JSON; result via `classify_edit_result` (string body → `metadata.diff` colored unified diff → `output` → JSON). `udiff_byte_styles` colors a diff blob per line class. Omits the web word-level `InlineDiff` (deferred to Phase 4).

## Source For
- [[Edit tool renders old new with syntax highlight]]
- [[Edit tool result renders unified diff from metadata]]
- [[TUI Udiff classify line precedence]]
