---
type: impl
source: src/tui/scenarios/spans/chips.rs
lang: rust
tags:
  - impl/generated
  - impl/rust
---
Inline-row chip extractors for the Spans tree:
- `shell_command_chips`: split on whitespace-bounded `&&`/`||`/`|`, first non-`KEY=value` token, basename, 24-char `…` truncate, 6 + overflow chip.
- `skill_chip`: when args is a non-array object with non-empty `skill` string.
- `report_intent_title`: when args is a non-array object with non-empty `intent` string.
- `tool_description_label`: when `args.description` is a non-empty string.
- `diff_stat(kind, args)`: edit/write/patch dispatch using the Copilot vendor for patch text; `count_lines` treats trailing `
` as terminator.

## Source For
- [[Shell command chip extracts primary words]]
- [[Skill name chip shows skill argument]]
- [[Report intent title shows on parent row]]
- [[Spans tool description inline label]]
- [[Spans diff stat badges on file mutation tools]]
- [[Copilot apply_patch diff-stat parsing]]
- [[TUI Spans chat row shows text preview from messages]]
- [[TUI Spans target chip renders file basename or URL host]]
- [[TUI Tool detail edit diff chip counts diff rows not block totals]]
