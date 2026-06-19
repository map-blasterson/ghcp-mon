---
type: impl
source: src/tui/scenarios/tool_detail/render_generic.rs
lang: rust
tags:
  - impl/generated
  - impl/rust
---
Generic fallback renderer (also used by external bodies): splits object args into "code-ish" string fields (those containing a newline) rendered as their own searchable blocks and the remaining structured args as one pretty-printed JSON block; renders a string result verbatim, else as JSON. Terminal port of the web `GenericArgs`.

## Source For
- [[Generic tool renders args splitting code-ish strings]]
- [[External tool detail uses generic args renderer]]
