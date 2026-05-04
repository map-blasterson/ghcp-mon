---
type: LLR
tags:
  - req/llr
  - domain/text-block
---
`TextBlock` MUST accept an optional `externalQuery` string prop. When `searchable` is truthy and `externalQuery` is a non-empty string, TextBlock MUST enter the `active` search phase with the query pre-populated and highlights applied, without requiring a user click to activate. When `externalQuery` becomes empty or undefined, TextBlock MUST exit the active phase and clear all highlights.

## Rationale
Span search needs to drive TextBlock highlighting programmatically. Reusing TextBlock's existing DOM-walk highlighting, match cycling, and cleanup avoids duplicating search logic.

## Derived from
- [[Searchable Text Block]]
