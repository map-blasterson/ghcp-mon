---
type: impl
source: src/tui/scenarios/file_touches/walk.rs
lang: rust
tags:
  - impl/generated
  - impl/rust
---
Pure "extract touches from a session span tree" helper. `extract_touches(tree, detail_lookup)` recurses every node (tool spans nest under invoke_agent), and for each `execute_tool` span resolves the cached detail, normalizes `projection.tool_call.tool_name` via `vendor::copilot::tool_name_mapping`, and keeps only `read`/`write`/`edit`/`patch` kinds. Path extraction goes through the vendor module — `resolve_argument(ArgConcept::FilePath)` for read/write/edit, `extract_apply_patch_paths` for patch — never hardcoded arg names (Phase 3 `b44e4ca` lesson). Read-kind → `TouchKind::Read`; write/edit/patch → `TouchKind::Write`. Missing detail, non-matching kind, missing arguments, and non-string paths are skipped silently.

## Source For
- [[File touches aggregates view edit create]]
- [[File touches builds filesystem tree with counts]]
- [[Copilot apply_patch path extraction]]
