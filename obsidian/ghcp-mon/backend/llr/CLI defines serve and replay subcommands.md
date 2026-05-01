---
type: LLR
tags:
  - req/llr
  - domain/cli
---
The `ghcp-mon` CLI MUST expose two subcommands: `serve` (start the OTLP receiver, REST API, and WebSocket) and `replay` (replay a JSON-lines telemetry file).

## Rationale
Operators need both live ingest and post-hoc replay from a single binary.

## Test context
- [[CLI Main Cheatsheet]]

## Derived from
- [[CLI Entry Point]]

## Test case
- [[CLI Main Tests]]
