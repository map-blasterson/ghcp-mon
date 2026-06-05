---
type: LLR
tags:
  - req/llr
  - domain/traces
---
A span tree row MUST display a distinct visual error indicator when its span carries an error — a non-null `error.type` and/or a non-OK `status_code` — so failed spans are discoverable in the tree without opening the detail view.

## Rationale
The span tree is the primary navigation surface; flagging errored rows lets the user spot failures at a glance, mirroring the row-level cues already carried by [[Placeholder ingestion state shown with rolling dots]] and [[Kind badge label renames raw kinds]]. `error.type` is the low-cardinality error-class string (e.g. `SessionDestroyedError`) and is distinct from the OTel `status_code` (Ok/Error/Unset) enum; either signal marks the row as errored. The error field rides into the row via [[API span responses include captured error type]].

## Derived from
- [[Trace and Span Explorer]]
