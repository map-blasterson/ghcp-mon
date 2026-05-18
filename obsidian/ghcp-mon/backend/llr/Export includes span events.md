---
type: LLR
tags:
  - req/llr
  - domain/export
---
For each exported span, `export::export_session` MUST attach every row from `span_events` matching the span's `span_pk` as the `events` field of the emitted `SpanEnvelope`, ordered by `time_unix_ns ASC, event_pk ASC`, with each event's `attributes_json` decoded into a JSON object (falling back to an empty object when the stored value is not a JSON object).

## Rationale
Replay-side normalization rebuilds projections from event payloads (hook/skill events, usage info); dropping events would lose derived state on round-trip.

## Derived from
- [[Session Export]]
