---
type: LLR
tags:
  - req/llr
  - domain/static
---
When the request path matches no embedded asset and `index.html` is also absent from the bundle, `static_handler` MUST return HTTP 404 with body `"not found"`.

## Rationale
Defensive: a build that ships without the SPA must surface that fact rather than serving empty bodies.

## Test context
- [[Static Assets Cheatsheet]]

## Derived from
- [[Embedded Dashboard SPA]]

## Test case
- [[Static Assets Tests]]
