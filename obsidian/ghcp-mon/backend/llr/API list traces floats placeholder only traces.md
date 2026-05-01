---
type: LLR
tags:
  - req/llr
  - domain/api
---
`GET /api/traces` MUST order results so traces whose computed `last_seen_ns` is 0 (placeholder-only traces with no timestamps) appear before traces with non-zero `last_seen_ns`, with ties within each group broken by `last_seen_ns DESC`.

## Rationale
A freshly-arrived trace whose root is still a placeholder is the newest data; under a plain DESC sort it would sink to the bottom.

## Test context
- [[REST API Cheatsheet]]

## Derived from
- [[Dashboard REST API]]

## Test case
- [[REST API Tests]]
