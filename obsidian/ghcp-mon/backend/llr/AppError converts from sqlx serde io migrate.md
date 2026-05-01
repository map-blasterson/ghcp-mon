---
type: LLR
tags:
  - req/llr
  - domain/error
---
`AppError` MUST provide `From` conversions for `sqlx::Error`, `sqlx::migrate::MigrateError`, `serde_json::Error`, and `std::io::Error` so that handlers can use the `?` operator on these error types directly.

## Rationale
Eliminates boilerplate `.map_err(...)` calls in every handler.

## Test context
- [[AppError Cheatsheet]]

## Derived from
- [[Uniform Error Reporting]]

## Test case
- [[AppError Tests]]
