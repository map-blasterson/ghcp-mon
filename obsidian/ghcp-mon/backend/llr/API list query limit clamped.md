---
type: LLR
tags:
  - req/llr
  - domain/api
---
The `limit` helper used by every API list endpoint MUST clamp the requested limit to the inclusive range `[1, max]`, defaulting to a per-endpoint default when the query parameter is absent.

## Rationale
Prevents pathological queries (`limit=0` or `limit=10_000_000`) regardless of which list endpoint receives them.

## Test context
- [[REST API Cheatsheet]]

## Derived from
- [[Dashboard REST API]]

## Test case
- [[REST API Tests]]
