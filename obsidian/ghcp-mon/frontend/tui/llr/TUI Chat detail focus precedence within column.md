---
type: LLR
tags:
  - req/llr
  - tui
  - domain/input-breakdown
---
Inside a focused Chat Detail column, focus MUST cycle (`Tab` / `Shift-Tab`) in a deterministic order over the tree row cursor first, then primitive key rows that carry truncatable content, then message-part body blocks. The `/` key activates per-block search ONLY when focus is on a primitive or part-body block (the tree-cursor focus delegates `/` to the column's external query). Within an Active per-block search, the text-input precedence layer consumes printable keys, `Backspace`, and `Esc` per [[TUI key-dispatch precedence text-input > modal > widget > column > global]].

## Rationale
Mirrors the Phase 3 tool-detail focus-plan idiom — keep the tree row as the primary cursor surface and surface block-level search on demand. The tree-cursor delegation to the column query keeps a single global search affordance for the typical "find this in the whole chat" interaction.

## Derived from
- [[Chat detail long primitives click to expand]]
- [[Detail columns pass span search query to TextBlocks]]
- [[TUI key-dispatch precedence text-input > modal > widget > column > global]]
