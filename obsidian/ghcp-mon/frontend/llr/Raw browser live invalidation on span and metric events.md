---
type: LLR
tags:
  - req/llr
  - domain/raw-browser
---
`RawBrowserScenario` MUST subscribe to `useLiveFeed([{ kind: "span", entity: "span" }, { kind: "metric", entity: "metric" }])` and MUST invalidate the `["raw", t]` query on every `tick`.

## Rationale
Both span and metric ingestion produce raw records; either can land while the user has the browser open.

## Derived from
- [[Raw Record Browser]]
