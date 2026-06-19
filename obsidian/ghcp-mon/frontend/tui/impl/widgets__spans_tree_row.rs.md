---
type: impl
source: src/tui/widgets/spans_tree_row.rs
lang: rust
tags:
  - impl/original
  - impl/rust
---
Per-row painter for the Spans session-span-tree, extracted from `App::draw_spans` so the row's cell layout (depth indent, collapse glyph, kind badge, placeholder rolling dots, name truncation, chips, description label, report-intent title) is one composable `Widget`. Purely a painter — chips/description/report-title/dim/highlight flags are computed by the caller. `display_name(node)` suppresses the name on Tool and Chat rows entirely; on InvokeAgent rows it prefers `projection.agent_run.agent_name`, then strips the `invoke_agent ` prefix, then returns empty when the name is exactly `invoke_agent`.

## Source For
- [[TUI Spans tree row layout in cells]]
- [[TUI Spans tree row suppresses noisy names]]
- [[TUI Spans chat row shows text preview from messages]]
