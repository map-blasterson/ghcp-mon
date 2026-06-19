---
type: impl
source: src/tui/widgets/markdown.rs
lang: rust
tags:
  - impl/generated
  - impl/rust
---
`markdown_to_lines(md, width) -> Vec<Line<'static>>`: pulldown-cmark (GFM) markdown renderer to styled ratatui lines. Headings → Yellow+BOLD with `#`; lists → `•`/`N.`; code → DarkGray; inline code → REVERSED; emphasis → ITALIC; strong → BOLD; links → underline + ` (url)`. Used by the task/read_agent renderers (display-only, not searchable).

## Source For
- [[Task tool renders prompt as markdown]]
- [[Read agent tool renders result as markdown]]
- [[TUI Markdown to lines via pulldown-cmark]]
