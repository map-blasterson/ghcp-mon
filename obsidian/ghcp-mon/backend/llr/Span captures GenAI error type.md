---
type: LLR
tags:
  - req/llr
  - domain/normalize
---
When normalizing a span, the normalizer MUST capture the span's `error.type` attribute (a conditionally-present, low-cardinality error-class string per the GenAI semantic conventions, e.g. `SessionDestroyedError`) and persist it on the span record, distinct from the OTel span `status_code`/`status_message`; when the attribute is absent (operation succeeded) the captured value is NULL.

## Rationale
`error.type` is the semconv error-class identifier describing how an observed GenAI client operation failed, and is generic to all GenAI client spans (not just chat). It is a separate signal from the OTel `status_code` (Ok/Error/Unset) enum already captured by [[Span upsert by trace and span id]]: `status_code` says whether the span errored, while `error.type` names the error class. Capturing it at span level keeps the low-cardinality failure reason queryable instead of leaving it buried in `attributes_json`.

## Test context
- [[Normalize Pipeline Cheatsheet]]

## Derived from
- [[Span Normalization]]

## Test case
- [[Normalize Pipeline Tests]]
