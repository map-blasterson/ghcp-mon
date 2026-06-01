---
type: LLR
tags:
  - req/llr
  - tui
  - domain/render
---
The TUI MUST render markdown bodies (the `task` prompt/result and `read_agent` result) to styled `Line`s via `markdown_to_lines(md, width)` using pulldown-cmark (GFM options). Mapping: ATX/Setext headings → `Yellow` + `BOLD` with a leading `#` marker; bullet list items → `•` prefix and ordered items → `N.` prefix; fenced/indented code → `DarkGray`; inline code → `REVERSED`; emphasis → `ITALIC`; strong → `BOLD`; links → underlined text followed by ` (url)`; hard/soft breaks → a new line. Output lines are `'static` so they may be appended into the column body buffer.

## Rationale
A bounded, dependency-light markdown-to-lines pass approximates the web `react-markdown` rendering inside terminal cells. Markdown bodies are flattened to text plus a per-byte base style vector and routed through `search_block`, so they participate in per-block `/` search and receive the column `external_query` like every other body block.

## Derived from
- [[Task tool renders prompt as markdown]]
- [[Read agent tool renders result as markdown]]
