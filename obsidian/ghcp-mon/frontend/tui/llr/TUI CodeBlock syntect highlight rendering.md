---
type: LLR
tags:
  - req/llr
  - tui
  - domain/render
---
The TUI MUST highlight source code via syntect: `syntect_byte_styles(text, lang)` returns one `Style` per byte of `text` when `lang` resolves to a known syntax (looked up by token/extension then case-insensitive name), else `None` (plain rendering). The cached `SyntaxSet` MUST use newline-aware defaults (`load_defaults_newlines`) and lines MUST be highlighted newline-terminated; the theme is `base16-ocean.dark`. Only foreground colour and font modifiers (bold/italic/underline) are carried into the ratatui `Style`; the syntect background MUST be dropped so the search-match overlay background remains visible. Highlight failures MUST degrade to plain text (logged at `warn`), never panic.

## Rationale
Per-byte styles let the unified line-builder compose syntect colouring with the search overlay in a single paint pass; dropping the theme background keeps yellow match cells legible.

## Derived from
- [[Edit tool renders old new with syntax highlight]]
- [[Code block highlights via Prism with extension map]]
