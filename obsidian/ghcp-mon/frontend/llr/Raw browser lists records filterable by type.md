---
type: LLR
tags:
  - req/llr
  - domain/raw-browser
---
`RawBrowserScenario` MUST query `api.listRaw({ type, limit: 200 })` (where `type` is `column.config.raw_type` or undefined), MUST render one row per `RawRecord` showing `record_type`, `#<id>`, and `received_at`, and MUST render the selected record's `body` via `JsonView` in a detail pane.

## Rationale
A direct surface over `/api/raw` for debugging the persistence layer.

## Derived from
- [[Raw Record Browser]]
