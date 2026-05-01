---
type: LLR
tags:
  - req/llr
  - domain/static
---
When serving an embedded asset, `static_handler` MUST set the response `Content-Type` header to the MIME type guessed from the asset path's extension (via `mime_guess`), defaulting to `application/octet-stream` when no guess is available.

## Rationale
Browsers need correct content types to load JS, CSS, fonts, and images; binary fallback prevents misrendering.

## Test context
- [[Static Assets Cheatsheet]]

## Derived from
- [[Embedded Dashboard SPA]]

## Test case
- [[Static Assets Tests]]
