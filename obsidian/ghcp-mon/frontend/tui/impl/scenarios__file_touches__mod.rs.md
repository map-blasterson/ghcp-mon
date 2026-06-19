---
type: impl
source: src/tui/scenarios/file_touches/mod.rs
lang: rust
tags:
  - impl/generated
  - impl/rust
---
Top-level File Touches scenario: `FileTouchesState` (open_dirs, known_dirs, focus_row, scroll_top + last-render row/dir snapshots), the `render` entry point (empty-state precedence → build tree → new-dir auto-open → header + scrollable tree paint), and `handle_key` for the column-layer keymap (`↑`/`↓`/`Home`/`End` cursor, `←`/`→`/`Space` dir toggle, `+`/`-` expand-all/collapse-all with no-op-when-empty). Live invalidation relies on Phase 0's cache table (`(span,span)` + `(derived,tool_call)` → `["session-span-tree", *]` / `["span", ...]`) and the draw-once-per-frame re-read.

## Source For
- [[File touches aggregates view edit create]]
- [[File touches new directories open by default]]
- [[File touches sort directories first then alphabetical]]
- [[File touches expand and collapse all controls]]
- [[File touches live invalidation on tool events]]
- [[TUI File touches tree row layout in cells]]
- [[TUI File touches header and bulk controls]]
- [[TUI File touches empty states]]
- [[TUI File touches preserves user collapse state across live updates]]
