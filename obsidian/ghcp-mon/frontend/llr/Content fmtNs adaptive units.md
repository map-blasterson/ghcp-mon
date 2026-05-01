---
type: LLR
tags:
  - req/llr
  - domain/input-breakdown
---
`fmtNs(ns)` MUST return `"—"` when `ns == null`; for `ns / 1_000_000 ≥ 1000` it MUST return `<seconds, two-decimal>"s"`; for `ms ≥ 1` it MUST return `<ms, one-decimal>"ms"`; otherwise it MUST return `<ns>"ns"`.

## Rationale
Adaptive units keep durations human-readable across nine orders of magnitude.

## Derived from
- [[Chat Input Breakdown]]
