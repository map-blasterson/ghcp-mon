---
type: LLR
tags:
  - req/llr
  - domain/replay
---
`ingest_jsonl_file` MUST log a warning and continue processing subsequent lines when a line fails to parse as a file-exporter envelope, rather than aborting the whole replay.

## Rationale
A single corrupt line in a long telemetry file should not invalidate the rest.

## Test context
- [[Ingest Pipeline Cheatsheet]]

## Derived from
- [[File Exporter Replay]]

## Test case
- [[Ingest Pipeline Tests]]
