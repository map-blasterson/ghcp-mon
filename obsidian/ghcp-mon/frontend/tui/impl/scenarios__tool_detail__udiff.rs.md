---
type: impl
source: src/tui/scenarios/tool_detail/udiff.rs
lang: rust
tags:
  - impl/generated
  - impl/rust
---
`udiff_classify(line) -> UdiffLine{Meta|Hunk|Add|Rem|Line}`: precedence-ordered unified-diff line classifier (`+++`/`---` and `Index:`/`====` → Meta; `@@` → Hunk; `+` → Add; `-` → Rem; else Line). The `+++`/`---` test precedes the single-char add/remove test. Terminal port of the web `UnifiedDiff` classifier.

## Source For
- [[Edit tool result renders unified diff from metadata]]
- [[TUI Udiff classify line precedence]]
