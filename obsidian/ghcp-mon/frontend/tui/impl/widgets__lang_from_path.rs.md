---
type: impl
source: src/tui/widgets/lang_from_path.rs
lang: rust
tags:
  - impl/generated
  - impl/rust
---
`lang_from_path(path) -> Option<&'static str>`: maps a file path to a syntect language token by extension, with special-filename overrides (`Dockerfile`, `Makefile`, `.gitignore`, …). Terminal port of the web `langFromPath` extension map.

## Source For
- [[Code block highlights via Prism with extension map]]
- [[TUI CodeBlock syntect highlight rendering]]
