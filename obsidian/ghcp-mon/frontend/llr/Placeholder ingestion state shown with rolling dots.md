---
type: LLR
tags:
  - req/llr
  - domain/traces
---
Any UI surface that renders a span row MUST display a `<RollingDots />` indicator inside a `tag warn` chip when the row's `ingestion_state === "placeholder"`, providing an animated visual cue that the row is awaiting real data.

## Rationale
Placeholder rows are an intentional ingest state; the dots make "still loading" visually distinct from "complete but empty".

## Derived from
- [[Trace and Span Explorer]]
