---
type: LLR
tags:
  - req/llr
  - domain/traces
---
`SpanDetailView` MUST render the span's error state — the captured `error.type` error-class string and/or a non-OK `status_code` — as a distinct, visually-flagged error element when either is present, and MUST omit the element entirely when the span did not error (no `error.type` and an Ok/Unset `status_code`).

## Rationale
Failed observed-session spans must be obvious in the inspector; a dedicated, flagged element separates "this operation errored" from the normal detail body. `error.type` is the low-cardinality error-class string (e.g. `SessionDestroyedError`) — distinct from the OTel `status_code` (Ok/Error/Unset) enum — surfaced to the client via [[API span responses include captured error type]]. Omitting the element when the span did not error keeps successful spans uncluttered.

## Derived from
- [[Trace and Span Explorer]]
