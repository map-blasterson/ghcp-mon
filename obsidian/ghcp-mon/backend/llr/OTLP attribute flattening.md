---
type: LLR
tags:
  - req/llr
  - domain/otlp
---
`flatten_otlp_attributes` MUST convert an OTLP attributes array (each element `{key, value: AnyValue}`) into a JSON object whose keys are the attribute keys and whose values are the unwrapped scalars/arrays/objects (`stringValue`, `intValue`, `doubleValue`, `boolValue`, `bytesValue`, `arrayValue.values` recursively, `kvlistValue.values` recursively).

## Rationale
Internal envelopes use a flat attribute map rather than the verbose AnyValue shape so downstream code can index attributes uniformly.

## Test context
- [[Ingest Pipeline Cheatsheet]]

## Derived from
- [[OTLP HTTP Receiver]]

## Test case
- [[Ingest Pipeline Tests]]
