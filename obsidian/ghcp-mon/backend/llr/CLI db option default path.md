---
type: LLR
tags:
  - req/llr
  - domain/cli
---
When the `--db <path>` global option is omitted, the CLI MUST default the SQLite database path to `./data/ghcp-mon.db`.

## Rationale
Local-first default keeps a stable on-disk location without requiring user configuration.

## Test context
- [[CLI Main Cheatsheet]]

## Derived from
- [[CLI Entry Point]]

## Test case
- [[CLI Main Tests]]
