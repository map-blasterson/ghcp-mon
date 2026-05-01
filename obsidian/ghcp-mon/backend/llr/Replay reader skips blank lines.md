---
type: LLR
tags:
  - req/llr
  - domain/replay
---
`ingest_jsonl_file` MUST skip lines whose trimmed content is empty without incrementing the ingest counter or producing an error.

## Rationale
Tolerate blank lines or trailing newlines in exporter output.

## Test context
- [[Ingest Pipeline Cheatsheet]]

## Derived from
- [[File Exporter Replay]]

## Test case
- [[Ingest Pipeline Tests]]
