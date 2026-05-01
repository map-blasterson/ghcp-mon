---
type: LLR
tags:
  - req/llr
  - domain/error
---
The HTTP response body for every `AppError` MUST be a JSON object of the form `{"error": "<message>"}`, where `<message>` is the variant's user-facing message (`BadRequest`/`NotImplemented` carry the supplied string, `NotFound` carries `"not found"`, and other variants use `Display`).

## Rationale
Uniform JSON shape is what the dashboard expects regardless of error origin.

## Test context
- [[AppError Cheatsheet]]

## Derived from
- [[Uniform Error Reporting]]

## Test case
- [[AppError Tests]]
