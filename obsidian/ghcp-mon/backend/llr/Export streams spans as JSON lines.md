---
type: LLR
tags:
  - req/llr
  - domain/export
---
`export::export_session(pool, conv_id, writer)` MUST serialize each selected span as a bare `SpanEnvelope` (with `kind_tag: "span"`) using `serde_json::to_string`, write each serialized envelope to `writer` followed by a single `
` byte, flush the writer at the end, and return `Ok(n)` where `n` is the count of envelopes written.

## Rationale
Replay reads the file-exporter format one envelope per line; the bare `SpanEnvelope` is dispatched by the `Envelope` enum's internal `type:"span"` tag, so a flat per-line JSON object is sufficient for round-tripping.

## Derived from
- [[Session Export]]
