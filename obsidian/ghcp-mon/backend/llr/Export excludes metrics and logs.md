---
type: LLR
tags:
  - req/llr
  - domain/export
---
`export::export_session` MUST NOT emit any metric-data-point or log envelopes; only span envelopes are written.

## Rationale
Spans are the canonical projection for the dashboard; metrics and logs are not required for replay and are intentionally excluded to keep the export format simple. (Per module docstring: metric/log replay can be added later if needed.)

## Derived from
- [[Session Export]]
