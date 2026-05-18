---
type: LLR
tags:
  - req/llr
  - domain/export
---
`export::export_session` MUST emit selected spans in ascending `start_unix_ns` order, with spans whose `start_unix_ns` is NULL emitted first, breaking ties by ascending `span_pk`.

## Rationale
Replay processes envelopes in order; emitting parents-before-children-ish (by start time) reduces the number of placeholders the normalizer must materialize during ingest.

## Derived from
- [[Session Export]]
