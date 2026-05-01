---
type: LLR
tags:
  - req/llr
  - domain/tool-detail
---
The `CodeBlock` component MUST render `Prism.highlight(text, grammar, language)` HTML when `Prism.languages[language]` resolves to a grammar, and MUST render HTML-escaped text otherwise. `langFromPath(path)` MUST return the matching slug from a fixed extension map (e.g. `ts → typescript`, `tsx → tsx`, `py → python`, `rs → rust`, `md → markdown`) or from a fixed filename map (`Dockerfile`, `Makefile`, `.gitignore`, `.bashrc`, `.zshrc` → `bash`), and MUST return `null` for unknown paths.

## Rationale
Centralizes language detection for tool-detail views; tree-shakes Prism by importing only the curated grammar set.

## Derived from
- [[Tool Call Inspector]]
