---
type: LLR
tags:
  - req/llr
  - domain/text-block
---
While in the `active` phase, `TextBlock` MUST render a sticky `<div class="tb-search-header" aria-hidden="true">` at the top of the block containing two spans: the left span MUST read `"${matchIndex + 1} of ${matchCount} matches"` when `query` is non-empty and `matchCount > 0`, and `"0 matches"` otherwise; the right span MUST read `"(shift)+LMB (prev)/(next)"` as a static keybinding hint.

## Rationale
The header gives the user immediate feedback about how many matches the current query produced and where they are in the cycle, plus a discoverable hint for the click-to-cycle affordance.

## Derived from
- [[Searchable Text Block]]
