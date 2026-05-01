---
type: LLR
tags:
  - req/llr
  - domain/api
---
`GET /api/raw` MUST return up to `limit` rows from `raw_records` ordered by `id DESC` (default 100, clamped to `[1, 500]`), filtered by `record_type` when the `type` query parameter is present. Each item MUST include `id`, `received_at`, `source`, `record_type`, `content_type`, and `body`, where `body` is parsed JSON when it is valid JSON and otherwise the raw string.

## Rationale
The raw-record explorer must be readable both for OTLP JSON bodies and free-form text.

## Test context
- [[REST API Cheatsheet]]

## Derived from
- [[Dashboard REST API]]

## Test case
- [[REST API Tests]]
