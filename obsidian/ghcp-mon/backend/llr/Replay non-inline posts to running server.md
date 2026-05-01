---
type: LLR
tags:
  - req/llr
  - domain/cli
  - domain/replay
---
When `replay` is invoked without `--inline`, the CLI MUST canonicalize the supplied path, POST a JSON body `{"path": "<canonical path>"}` to `<server>/api/replay` (where `<server>` defaults to `http://127.0.0.1:4319` and any trailing slash is stripped), and print the resulting status code and response body.

## Rationale
Non-inline replay drives a running server so dashboard clients see the events live.

## Test context
- [[CLI Main Cheatsheet]]

## Derived from
- [[CLI Entry Point]]
- [[File Exporter Replay]]

## Test case
- [[CLI Main Tests]]
