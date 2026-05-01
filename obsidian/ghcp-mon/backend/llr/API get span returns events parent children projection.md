---
type: LLR
tags:
  - req/llr
  - domain/api
---
`GET /api/spans/:trace_id/:span_id` MUST return HTTP 404 when no span matches; on hit the response MUST include a `span` object (with attributes and resource parsed from JSON), an `events` array (in `time_unix_ns ASC` order, each with parsed attributes), the `parent` span (if any), the immediate `children` ordered by start time, and the span's `projection` block.

## Rationale
The span detail panel needs everything in one round-trip to render without follow-up calls.

## Test context
- [[REST API Cheatsheet]]

## Derived from
- [[Dashboard REST API]]
- [[Uniform Error Reporting]]

## Test case
- [[REST API Tests]]
