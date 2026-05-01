---
type: LLR
tags:
  - req/llr
  - domain/api-client
---
The `qs` helper used to build URL query strings MUST omit any entry whose value is `undefined`, `null`, or the empty string, MUST `encodeURIComponent` both keys and values, MUST join entries with `&`, and MUST return the empty string when no entries remain (no leading `?`).

## Rationale
Skipping empty values keeps backend handlers from receiving stray `param=` flags and avoids spurious cache-key churn in TanStack Query.

## Derived from
- [[REST API Client]]
