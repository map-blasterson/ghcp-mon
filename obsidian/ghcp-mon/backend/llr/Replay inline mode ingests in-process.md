---
type: LLR
tags:
  - req/llr
  - domain/cli
  - domain/replay
---
When `replay` is invoked with `--inline`, the CLI MUST open the database, build an in-process `AppState`, call `ingest_jsonl_file` against the given path, and print the number of envelopes ingested without contacting any HTTP server.

## Rationale
Inline replay is used for tests and offline reconstruction without running a server.

## Test context
- [[CLI Main Cheatsheet]]

## Derived from
- [[CLI Entry Point]]
- [[File Exporter Replay]]

## Test case
- [[CLI Main Tests]]
