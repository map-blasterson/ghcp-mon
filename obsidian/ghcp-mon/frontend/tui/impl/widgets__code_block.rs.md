---
type: impl
source: src/tui/widgets/code_block.rs
lang: rust
tags:
  - impl/generated
  - impl/rust
---
`CodeBlock` ratatui widget + `syntect_byte_styles(text, lang) -> Option<Vec<Style>>`: syntect-based syntax highlighting returning one `Style` per byte (fg + font modifiers only; theme background dropped so the search overlay stays visible). OnceLock-cached `SyntaxSet` (`load_defaults_newlines`) + `base16-ocean.dark` theme; lines highlighted newline-terminated; failures degrade to plain (logged `warn`), never panic.

## Source For
- [[Edit tool renders old new with syntax highlight]]
- [[Code block highlights via Prism with extension map]]
- [[TUI CodeBlock syntect highlight rendering]]
