---
type: LLR
tags:
  - req/llr
  - domain/replay
---
`POST /api/replay` MUST accept a JSON body `{"path": "<filesystem path>"}`, run `ingest_jsonl_file` against that path with `source='replay'`, and return `{"path": "<input path>", "ingested": <count>}` on success.

## Rationale
The replay endpoint is the bridge between the CLI's non-inline mode and the live server.

## Test context
- [[Replay Endpoint Cheatsheet]]

## Derived from
- [[File Exporter Replay]]

## Test case
- [[Replay Endpoint Tests]]
