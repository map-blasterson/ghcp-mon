---
type: HLR
tags:
  - req/hlr
  - domain/cli
---
The binary provides a command-line interface for operators to start the telemetry server and to replay archived telemetry files, with global configuration (such as the SQLite database path) shared across subcommands.

## Derived LLRs
- [[CLI defines serve and replay subcommands]]
- [[CLI db option default path]]
- [[CLI session state dir flag overrides default]]
- [[Serve binds OTLP and API listeners]]
- [[Replay inline mode ingests in-process]]
- [[Replay non-inline posts to running server]]
- [[CLI initializes tracing subscriber]]
