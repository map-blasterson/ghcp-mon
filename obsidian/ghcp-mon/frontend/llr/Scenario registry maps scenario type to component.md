---
type: LLR
tags:
  - req/llr
  - domain/workspace
---
`web/src/scenarios/index.ts` MUST export a `SCENARIOS` record whose keys are exactly the values of `ScenarioType` and whose values are the corresponding scenario component constructors.

## Rationale
Centralized registry consumed by alternate dispatchers/tests; complements `ColumnBody`'s switch.

## Derived from
- [[Workspace Layout]]
