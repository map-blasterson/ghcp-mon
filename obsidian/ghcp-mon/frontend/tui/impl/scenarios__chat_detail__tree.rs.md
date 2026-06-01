---
type: impl
source: src/tui/scenarios/chat_detail/tree.rs
lang: rust
tags:
  - impl/generated
  - impl/rust
---
Pure tree construction: `NodeId` (slash-delimited path newtype), `ChatMode` (DELTA/FULL with `from_config_str` + `toggled`), `NodeBadge` (Unchanged/Changed/Added/Removed), `NodeKind` (Root, Section, SystemParts, SystemUnchanged, SystemChanged, SystemDiff(Vec<DiffSegment>), ToolDef, ToolDefUnchanged, ToolDefAdded, ToolDefRemoved, InputMessagesUnchanged, Message, Part, Leaf — one variant per visual branch, no bg-color sentinels), `TreeNode` (id/label/meta/bytes/badge/kind/children/primitives), `ChatContent` (the 4 captured arrays, with `from_attrs` and `is_empty`). `build_tree` is the entry point; per-branch helpers `build_system_node` (word-diff via `similar::TextDiff::from_words`), `build_tool_defs_node` (multiset name-diff with REMOVED-before-ADDED ordering), `build_input_messages_node` (carried-forward suffix detection preserving original-array indices), `build_output_messages_node`, and `build_message_node`. `json_bytes` is `serde_json::to_string(&v).map(|s| s.len()).unwrap_or(0)` per the LLR fallback rule.

## Source For
- [[Chat detail tree built from four content attributes]]
- [[Chat detail bytes computed via JSON length]]
- [[Chat detail DELTA diffs against prior chat span]]
- [[Chat detail system instructions word-diff in DELTA]]
- [[Chat detail tool defs name-diff in DELTA]]
- [[Chat detail DELTA input messages carried-forward suffix]]
- [[Chat detail mode toggle DELTA FULL]]
- [[TUI Chat detail node id is slash-delimited path]]
