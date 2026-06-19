---
type: LLR
tags:
  - req/llr
  - tui
  - domain/spans
---
`spans_tree_row::display_name(node)` MUST return an empty string when `node.is_tool_row()` is true OR `node.kind_class == Chat`, and for `KindClass::InvokeAgent` rows MUST prefer the server-projected `node.projection.agent_run.agent_name` when present, otherwise the suffix of `node.name` after stripping the `"invoke_agent "` prefix, otherwise an empty string when the name is exactly `"invoke_agent"`. The `SpansTreeRow` widget MUST consume this name verbatim — no further stripping in the row painter.

## Rationale
The kind badge already says "tool"/"chat"/"agent"; the raw span name is almost always a noisy duplicate ("execute_tool bash", "invoke_agent rg", "chat"). The interesting per-row content is the chips and the description/preview.

## Derived from
- [[TUI Spans tree row layout in cells]]
