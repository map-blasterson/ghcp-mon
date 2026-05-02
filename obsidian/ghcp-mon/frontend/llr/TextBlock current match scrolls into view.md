---
type: LLR
tags:
  - req/llr
  - domain/text-block
---
After (re-)highlighting matches in the `active` phase, when `matchCount > 0` `TextBlock` MUST add the class `tb-match-current` to the mark at the clamped `matchIndex` (`Math.min(matchIndex, matchCount - 1)`) and call `scrollIntoView({ block: "nearest" })` on it; if the previous `matchIndex` exceeds the new `matchCount - 1`, `TextBlock` MUST clamp `matchIndex` to that bound.

## Rationale
The current match must remain visible inside its scroll container as the user cycles or types.

## Derived from
- [[Searchable Text Block]]
