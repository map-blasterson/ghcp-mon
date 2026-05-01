---
type: LLR
tags:
  - req/llr
  - domain/replay
---
`parse_file_exporter_line` MUST deserialize a JSON object whose `type` field is one of `"span"`, `"metric"`, or `"log"` into the corresponding `Envelope::Span`/`Envelope::Metric`/`Envelope::Log` variant, returning an `AppError::BadRequest` if the input does not parse.

## Rationale
The file-exporter format uses an externally-tagged discriminant to distinguish envelope kinds.

## Test context
- [[Ingest Pipeline Cheatsheet]]
- [[Model Envelope Cheatsheet]]

## Derived from
- [[File Exporter Replay]]

## Test case
- [[Ingest Pipeline Tests]]
