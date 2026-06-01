---
type: LLR
tags:
  - req/llr
  - tui
  - domain/traces
---
When `column.config.session` is unset, the TUI Spans column body renders a recent-traces list (instead of the session span tree).

Source: `cache_get(["traces"], 5_000ms, || api.list_traces({ limit: 50 }))`. Each `TraceSummary` produces one row in this cell order:

1. First 8 chars of `trace_id` (monospaced).
2. Two-cell gap, then `fmt_relative(last_seen_ns)`.
3. Two-cell gap, then `spans:<span_count>`.
4. Two-cell gap, then per-kind counts: `chat:N tool:N ext:N agent:N other:N`.

The focused row receives the same cyan-background + bold-black highlight used in the tree mode. Arrow keys move the cursor; `Enter` is reserved for Phase 6 trace-pick.

Kind filter dim — per [[Traces list dims rows below kind filter]]: when
`config.kind_filter` is set, every row whose `kind_counts[kind_filter] ==
0` receives `Modifier::DIM` across the full row; matching rows render at
full intensity. Rows are NEVER hidden — only dimmed — so tree-list context
is preserved.

Switching `config.session` to a non-empty value swaps the column to tree
mode and clears `selected_trace_id` / `selected_span_id`, per
[[Spans scenario two modes session vs traces]].

## Rationale
A traces list is the entry point before any session is pinned; the same
cache key (`["traces"]`) the web UI consults keeps both clients in sync
under live invalidations.

## Derived from
- [[Spans scenario two modes session vs traces]]
- [[Traces list dims rows below kind filter]]
