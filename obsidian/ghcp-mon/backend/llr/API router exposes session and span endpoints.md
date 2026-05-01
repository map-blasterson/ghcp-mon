---
type: LLR
tags:
  - req/llr
  - domain/api
---
The API router MUST mount the following GET routes — `/api/healthz`, `/api/sessions`, `/api/sessions/:cid`, `/api/sessions/:cid/span-tree`, `/api/sessions/:cid/contexts`, `/api/sessions/:cid/registries`, `/api/spans`, `/api/spans/:trace_id/:span_id`, `/api/traces`, `/api/traces/:trace_id`, `/api/raw`, `/ws/events` — the POST route `/api/replay`, and the DELETE route `/api/sessions/:cid`, with a fallback that serves the embedded SPA.

## Rationale
A single router pins down the URL surface that dashboard clients depend on.

## Test context
- [[REST API Cheatsheet]]

## Derived from
- [[Dashboard REST API]]

## Test case
- [[REST API Tests]]
