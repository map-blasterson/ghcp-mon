---
type: LLR
tags:
  - req/llr
  - domain/api
---
The API router MUST install a CORS layer that allows any origin, any method, and any headers.

## Rationale
Local-first usage: the dashboard frontend may be served from any localhost port, so permissive CORS is intentional.

## Test context
- [[REST API Cheatsheet]]

## Derived from
- [[Dashboard REST API]]

## Test case
- [[REST API Tests]]
