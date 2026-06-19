---
type: LLR
tags:
  - req/llr
  - tui
  - domain/spans
---
For ToolKind::Edit, `chips::diff_stat(kind, args)` MUST compute the +/- chip counts by invoking `tool_detail::inline_diff::build(old_str, new_str)` and counting Add and Rem rows via `inline_diff::count_changes`, NOT by reporting `(count_lines(new_str), count_lines(old_str))`. For ToolKind::Write the chip MUST be `(count_lines(file_text), 0)`. For ToolKind::Patch the chip MUST be `vendor::copilot::apply_patch_diff_stat(text)`. All other kinds MUST be `(0, 0)`. `count_lines` MUST treat a trailing `
` as a terminator (`"foo
"` = 1 line) and the empty string as 0 lines.

## Rationale
The old implementation over-counted a one-line touch in a many-line block as `+N -N`; routing through the real line-level diff fixes the regression.

## Derived from
- [[Spans diff stat badges on file mutation tools]]
