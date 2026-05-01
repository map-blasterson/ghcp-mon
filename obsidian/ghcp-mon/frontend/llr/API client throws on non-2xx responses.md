---
type: LLR
tags:
  - req/llr
  - domain/api-client
---
When `fetch` returns a response with `ok === false`, the API client's `getJson`/`deleteJson` helpers MUST throw an `Error` whose message is `"<status> <statusText> for <path>"`, and MUST NOT attempt to parse the response body.

## Rationale
React Query relies on a thrown error to mark a query as failed; using `r.statusText` keeps the surfaced message human-readable in error toasts.

## Derived from
- [[REST API Client]]
