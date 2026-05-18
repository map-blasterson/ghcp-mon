---
type: LLR
tags:
  - req/llr
  - domain/cli
  - domain/export
---
The `ghcp-mon export <session>` subcommand MUST open the configured SQLite database, call `export::export_session` for the supplied `gen_ai.conversation.id`, and write the resulting JSON-lines stream to the file given by `-o/--output <path>` when present (created/truncated via `tokio::fs::File::create` and wrapped in a `BufWriter`) or to `stdout` otherwise. On success it MUST print a human-readable count summary to stderr (e.g. `exported N spans` or `exported N spans to <path>`).

## Rationale
The export is designed to be pipeable (`ghcp-mon export ... | ghcp-mon replay /dev/stdin`); routing the count summary to stderr keeps stdout a clean JSON-lines stream.

## Derived from
- [[Session Export]]
- [[CLI Entry Point]]
