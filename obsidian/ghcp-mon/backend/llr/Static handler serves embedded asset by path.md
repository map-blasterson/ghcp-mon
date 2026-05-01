---
type: LLR
tags:
  - req/llr
  - domain/static
---
`static_handler` MUST treat the request URI's path (with the leading `/` stripped, mapping the empty path to `index.html`) as a key into the embedded `web/dist/` asset bundle and, when a matching asset exists, return HTTP 200 with that file's bytes.

## Rationale
Single-binary deployment: the SPA is served straight from the embedded bundle.

## Test context
- [[Static Assets Cheatsheet]]

## Derived from
- [[Embedded Dashboard SPA]]

## Test case
- [[Static Assets Tests]]
