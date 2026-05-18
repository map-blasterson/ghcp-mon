---
type: LLR
tags:
  - req/llr
  - domain/cli
---
The `tracing_subscriber` `fmt` layer installed at startup MUST be configured with `with_writer(std::io::stderr)` so that all diagnostic log output is emitted to stderr and never to stdout.

## Rationale
Subcommands such as `ghcp-mon export` write structured payloads (JSON-lines) to stdout for pipe consumers; routing logs to stderr keeps stdout a clean machine-readable stream.

## Derived from
- [[CLI Entry Point]]
