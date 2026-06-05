---
type: LLR
tags:
  - req/llr
  - domain/api
---
The span API MUST include the captured `error.type` value as an explicit field on every serialized span — the rich `span` object returned by `GET /api/spans/:trace_id/:span_id` (beside the existing `status_code`/`status_message`), each span-tree node, and each `GET /api/spans` row — set to the persisted error-class string when the span errored and `null` when it did not, so clients render error state without parsing the raw `attributes_json` blob.

## Rationale
The presentation layer (the span-detail error block and the span-tree row error indicator, on both the web dashboard and the TUI) needs the error class in the span payload it already fetches; exposing it as a first-class field avoids re-parsing `attributes_json` on the client. `error.type` is the low-cardinality error-class string (e.g. `SessionDestroyedError`) captured by [[Span captures GenAI error type]] and is distinct from the OTel `status_code` (Ok/Error/Unset) enum: `status_code` says whether the span errored, while `error.type` names the failure class. Both may be present and are exposed independently.

## Test context
- [[REST API Cheatsheet]]

## Derived from
- [[Dashboard REST API]]

## Test case
- [[REST API Tests]]
