---
type: impl
source: src/tui/scenarios/tool_detail/render_view.rs
lang: rust
tags:
  - impl/generated
  - impl/rust
---
View/read renderer: path kv + extra args JSON; result body resolved from a string or `output`, unwrapped from the opencode `<path>/<type>/<content>` XML envelope (`strip_xml_wrapper`), then `strip_line_numbers` splits `N. `/`N: ` prefixes into a dim left gutter (regex-free) with the remaining source syntect-highlighted by extension.

## Source For
- [[View tool splits line numbers into gutter]]
