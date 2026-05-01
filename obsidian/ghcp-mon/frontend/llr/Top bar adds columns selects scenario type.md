---
type: LLR
tags:
  - req/llr
  - domain/workspace
---
The top bar MUST render a `<select>` whose options are `Object.entries(SCENARIO_LABELS)`; selecting an option MUST append a new `Column` to the workspace with that scenario type, an id from `genId()`, the matching label as title, an empty `config`, and `width: 1.2`, and MUST then reset the select's value to the empty placeholder.

## Rationale
Adding columns is the primary user-facing workspace mutation; the placeholder reset returns the control to its idle state.

## Derived from
- [[Workspace Layout]]
