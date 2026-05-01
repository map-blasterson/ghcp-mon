---
type: LLR
tags:
  - req/llr
  - domain/cli
---
On startup the binary MUST initialize a `tracing_subscriber` registry honoring the `RUST_LOG` env filter, falling back to the filter `info,sqlx=warn,tower_http=warn,hyper=warn` when none is set.

## Rationale
Predictable default verbosity prevents noisy framework logs from drowning application logs.

## Test context
- [[CLI Main Cheatsheet]]

## Derived from
- [[CLI Entry Point]]

## Test case
- [[CLI Main Tests]]
