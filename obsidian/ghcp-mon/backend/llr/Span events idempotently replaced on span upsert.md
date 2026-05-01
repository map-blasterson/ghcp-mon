---
type: LLR
tags:
  - req/llr
  - domain/normalize
---
On every span upsert (insert or conflict path), the normalizer MUST `DELETE FROM span_events WHERE span_pk = ?` for the upserted span and then re-insert exactly the events carried on the current envelope. Replays of the same span MUST therefore not accumulate duplicate `span_events` rows.

## Rationale
Spans are the canonical truth and may be re-delivered; the events table must converge to the latest delivery rather than grow with each replay. Deletion is scoped by `span_pk` so other spans' events are untouched.

## Test context
- [[Normalize Pipeline Cheatsheet]]

## Derived from
- [[Span Normalization]]

## Test case
- [[Normalize Pipeline Tests]]
