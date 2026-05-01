---
type: LLR
tags:
  - req/llr
  - domain/cli
---
The `serve` subcommand MUST bind two TCP listeners — the OTLP receiver at the address given by `--otlp-addr` (default `127.0.0.1:4318`) and the REST API + WebSocket at the address given by `--api-addr` (default `127.0.0.1:4319`) — and run both concurrently until either fails.

## Rationale
Separating OTLP and dashboard ports lets operators expose them with different network policies.

## Test context
- [[CLI Main Cheatsheet]]
- [[Server Router Cheatsheet]]

## Derived from
- [[CLI Entry Point]]

## Test case
- [[CLI Main Tests]]
