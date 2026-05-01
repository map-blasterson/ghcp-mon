---
type: LLR
tags:
  - req/llr
  - domain/input-breakdown
---
The `attrs(span)` helper MUST return `span.attributes` when it is a non-null object, MUST otherwise parse `span.attributes_json` as JSON and return it when it parses to a plain object, and MUST return an empty object in every other case.

## Rationale
Some call sites carry the live API shape (`attributes`); others carry the raw DB shape (`attributes_json`).

## Derived from
- [[Chat Input Breakdown]]
