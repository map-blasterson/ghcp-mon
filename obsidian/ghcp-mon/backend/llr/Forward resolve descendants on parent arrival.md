---
type: LLR
tags:
  - req/llr
  - domain/normalize
---
After resolving its own projection pointers, the normalizer MUST recursively (up to depth 128) re-resolve projection pointers for every descendant span of the just-ingested span, so children that were ingested before their parent get their pointers populated when the parent finally arrives.

## Rationale
Out-of-order ingest is normal; descendants must be reconciled when an ancestor that completes their lineage shows up.

## Test context
- [[Normalize Pipeline Cheatsheet]]

## Derived from
- [[Span Normalization]]

## Test case
- [[Normalize Pipeline Tests]]
