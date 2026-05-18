---
type: LLR
tags:
  - req/llr
  - domain/cli
  - domain/export
---
When the `<session>` argument passed to `ghcp-mon export` does not correspond to any row in the `sessions` table, the binary MUST print `session not found: <session>` to stderr and exit with a non-zero status code (specifically `1` via `std::process::exit(1)`), without creating or truncating the `-o/--output` file.

## Rationale
A missing session must not produce an empty or truncated artifact; failing fast before opening the output protects existing files at that path.

## Derived from
- [[Session Export]]
- [[CLI Entry Point]]
