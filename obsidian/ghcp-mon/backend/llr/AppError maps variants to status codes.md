---
type: LLR
tags:
  - req/llr
  - domain/error
---
When converting an `AppError` into an HTTP `Response`, the implementation MUST map `BadRequest` to `400`, `NotFound` to `404`, `NotImplemented` to `501`, and every other variant (`Sqlx`, `Migrate`, `Json`, `Io`, `Other`) to `500`.

## Rationale
Single mapping table keeps client error handling deterministic.

## Test context
- [[AppError Cheatsheet]]

## Derived from
- [[Uniform Error Reporting]]

## Test case
- [[AppError Tests]]
