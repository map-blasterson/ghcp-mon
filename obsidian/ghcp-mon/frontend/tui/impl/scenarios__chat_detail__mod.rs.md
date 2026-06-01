---
type: impl
source: src/tui/scenarios/chat_detail/mod.rs
lang: rust
tags:
  - impl/generated
  - impl/rust
---
Chat-detail scenario entry: `ChatDetailState` (expanded sets, primitive expansion, search-expansion snapshot, focus row, scroll, focus map for key dispatch, per-block text-block states), `render(...)` (resolves empty/no-content/non-chat states, builds the tree, reconciles search-expansion lifecycle, applies tool-call hint auto-expansion, paints header + summary bar + scrollable tree), and `handle_key(...)` (Up/Down focus row, Left/Right collapse/expand via the captured focus map, Space toggle for node or primitive, `m` flips DELTA↔FULL, Home/End jump). Row builder walks the tree depth-first and emits typed `RenderRow`s with arrow gutter, focus glyph, indent, collapse glyph, badge, label (with optional per-segment styling for SystemDiff), expand indicator `(+)`/`(-)`, and meta. `paint_with_search` overlays case-insensitive matches with a yellow/black highlight. `color_for_node` assigns per-branch colours (system=blue, tools=yellow, input=green, output=orange).

## Source For
- [[Chat detail only renders for chat span]]
- [[Chat detail tree built from four content attributes]]
- [[Chat detail summary bar proportional to visible segments]]
- [[Chat detail mode toggle DELTA FULL]]
- [[Chat detail DELTA diffs against prior chat span]]
- [[Chat detail tool-call hint auto-expand and arrow]]
- [[Chat detail key cursor icon follows pointer]]
- [[Chat detail long primitives click to expand]]
- [[ChatDetail auto-expands tree to span search matches]]
- [[Detail columns pass span search query to TextBlocks]]
- [[Message view renders parts by type]]
- [[TUI Chat detail layout in cells]]
- [[TUI Chat detail tool-call arrow gutter]]
- [[TUI Chat detail key-cursor indicator on focused key row]]
- [[TUI Chat detail node id is slash-delimited path]]
- [[TUI Chat detail focus precedence within column]]
- [[TUI Chat detail mode chip in header]]
- [[TUI Chat detail search-expanded set tracks restoration]]
