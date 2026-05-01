---
type: LLR
tags:
  - req/llr
  - domain/normalize
---
`handle_envelope` MUST NOT produce any normalized rows for `Envelope::Log` envelopes; it returns `Ok(())` after no-op handling, leaving log persistence to the raw-record archive only.

## Rationale
Logs are accepted and archived, but normalization for them is intentionally deferred.

## Test context
- [[Normalize Pipeline Cheatsheet]]

## Derived from
- [[Span Normalization]]

## Test case
- [[Normalize Pipeline Tests]]
