---
type: impl
source: src/tui/scenarios/file_touches/tree.rs
lang: rust
tags:
  - impl/generated
  - impl/rust
---
Pure counting-tree builder. `build_tree(touches)` splits each touch path on `/` (dropping empty segments to collapse repeated/leading/trailing separators), walks/creates a `TouchNode` per segment, increments `reads`/`writes` on every ancestor and the leaf, and appends the `TouchRef` to the leaf's `file_touches`. Children at every level are sorted directories-first (`is_dir` = explicit Dir kind OR has children) then case-insensitive alphabetical by name. `dir_paths` collects every directory path pre-order for the open-dir set and bulk controls.

## Source For
- [[File touches builds filesystem tree with counts]]
- [[File touches sort directories first then alphabetical]]
- [[TUI File touches tree row layout in cells]]
