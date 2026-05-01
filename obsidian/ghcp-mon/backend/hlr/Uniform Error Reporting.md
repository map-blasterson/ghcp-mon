---
type: HLR
tags:
  - req/hlr
  - domain/error
---
All HTTP handlers report failures using a single application error type that maps each error variant to an appropriate HTTP status code and a JSON body, so clients receive consistent error shapes regardless of the failure origin (database, JSON parsing, IO, validation).

## Derived LLRs
- [[AppError maps variants to status codes]]
- [[AppError JSON body contains error message]]
- [[AppError converts from sqlx serde io migrate]]
