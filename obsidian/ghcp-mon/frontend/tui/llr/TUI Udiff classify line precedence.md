---
type: LLR
tags:
  - req/llr
  - tui
  - domain/render
---
`udiff_classify(line)` MUST classify a unified-diff line with this precedence (highest first): a `+++` or `---` prefix → `Meta`; else a `@@` prefix → `Hunk`; else an `Index:` or `====` prefix → `Meta`; else a leading `+` → `Add`; else a leading `-` → `Rem`; else → `Line`. The `+++`/`---` file-header test MUST take precedence over the single-character add/remove test, so a bare `"+++"` classifies as `Meta`, not `Add`. The edit/write result renderer MUST colour each diff line by class (Meta dim-cyan, Hunk bold-magenta, Add green, Rem red, Line default) by emitting one base `Style` per byte for the whole blob.

## Rationale
Precedence-ordered prefix matching reproduces the web `UnifiedDiff` classifier exactly so multi-character markers are never misread as single-character add/remove lines.

## Derived from
- [[Edit tool result renders unified diff from metadata]]
