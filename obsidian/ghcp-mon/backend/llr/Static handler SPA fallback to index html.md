---
type: LLR
tags:
  - req/llr
  - domain/static
---
When no embedded asset matches the request path, `static_handler` MUST fall back to serving `index.html` so the SPA's client-side router can handle the URL.

## Rationale
Deep links into client routes (`/sessions/abc`) must not 404 just because no static file matches.

## Test context
- [[Static Assets Cheatsheet]]

## Derived from
- [[Embedded Dashboard SPA]]

## Test case
- [[Static Assets Tests]]
