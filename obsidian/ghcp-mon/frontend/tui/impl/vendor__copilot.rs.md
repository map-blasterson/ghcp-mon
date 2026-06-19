---
type: impl
source: src/tui/vendor/copilot.rs
lang: rust
tags:
  - impl/generated
  - impl/rust
---
Copilot tool-call adapter: tool_name_mapping, resolve_argument (with the three-step patch_text fallback), result_envelope (string-only body_string), extract_apply_patch_paths (Add/Update/Delete File + Move to), apply_patch_diff_stat (+/- counting, header skip).

## Source For
- [[Copilot tool-name mapping]]
- [[Copilot tool-call argument mapping]]
- [[Copilot tool-call result envelope]]
- [[Copilot apply_patch path extraction]]
- [[Copilot apply_patch diff-stat parsing]]
