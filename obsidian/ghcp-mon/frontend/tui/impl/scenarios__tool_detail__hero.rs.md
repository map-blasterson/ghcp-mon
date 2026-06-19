---
type: impl
source: src/tui/scenarios/tool_detail/hero.rs
lang: rust
tags:
  - impl/generated
  - impl/rust
---
`hero_value(args, tool_type) -> Option<String>`: selects the hero-panel value from priority keys `["command", "query", "description"]`, only for `tool_type == "function"` with a non-null object whose first present key renders non-empty (strings verbatim, others via `pretty_json`). Terminal port of the web `pickHero`.

## Source For
- [[Tool detail hero panel surfaces key argument]]
