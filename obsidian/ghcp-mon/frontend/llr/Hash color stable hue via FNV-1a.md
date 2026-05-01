---
type: LLR
tags:
  - req/llr
  - domain/traces
---
`hashColor(s)` MUST compute a 32-bit FNV-1a hash of `s` (offset basis `0x811c9dc5`, prime `0x01000193`), MUST take its `% 360` as a hue, and MUST return the CSS string `hsl(<hue>, 65%, 68%)`. The same input string MUST always produce the same colour across reloads.

## Rationale
Stable per-string hue lets users mentally map tool/agent names to colours.

## Derived from
- [[Trace and Span Explorer]]
