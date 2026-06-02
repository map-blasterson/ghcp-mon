---
type: impl
source: src/tui/scenarios/tool_detail/mod.rs
lang: rust
tags:
  - impl/generated
  - impl/rust
---
Tool-detail scenario entry: `ToolDetailState` (metadata/raw-attrs open flags, scroll, focus plan, per-block search states), `FocusKind`, the unified searchable line-builder (`build_block_lines` — soft-wrap + gutter modes, reuses `wrap_text`/`locate_matches`, paints per-byte base style with yellow/orange match overlay), the `BodyCtx` accumulator (label/kv/search/code/json/markdown/metadata/json-panel/no-content), `render(...)` (resolves selection + search query + cached detail, dispatches via `dispatch::route`, blits with column scroll + scroll-into-view) and `handle_key(...)` (Tab/BackTab cycle, scroll, Space toggles, `/` search with text-input precedence). Empty/loading/not-a-tool states. Tests live in the sibling `tests` module.

## Source For
- [[Tool detail requires tool call projection]]
- [[Tool detail prefers native tool call over external]]
- [[Tool detail empty state when no content captured]]
- [[Tool detail body blocks wrap in TextBlock for search]]
- [[Detail columns pass span search query to TextBlocks]]
- [[Tool detail metadata panel collapsible]]
- [[Tool detail hero panel surfaces key argument]]
- [[Column body dispatches by scenario type]]
- [[TUI Tool detail bottom-up layout in cells]]
- [[TUI Tool detail key-dispatch precedence within column]]
- [[TUI Tool detail metadata panel default closed]]
- [[TUI Tool detail empty state verbatim copy]]
- [[TUI Tool detail edit inline diff renders unified rows]]
- [[Tool detail inline diff for edit tool]]
