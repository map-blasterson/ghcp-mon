---
type: LLR
tags:
  - req/llr
  - domain/context-growth
---
For each chat span, `ContextGrowthWidget` MUST merge the per-`span_pk` `context_snapshots` returned by `api.listSessionContexts(session)` into a single `MergedRow` `{ span_pk, token_limit, input_tokens, output_tokens, reasoning_tokens, cache_read_tokens, latest_ns }` using the rule: (a) ignore snapshots whose `span_pk` is null or whose `span_pk` is not present in the session's chat-span map (i.e., the row is keyed on `kind_class === "chat"` spans only); (b) take `token_limit` as the maximum non-null `token_limit` observed across the span's snapshots; (c) for `input_tokens` / `output_tokens` / `reasoning_tokens` / `cache_read_tokens`, prefer the value from the snapshot with the largest `captured_ns` (overwriting only when non-null), and otherwise back-fill any field that is still null from any earlier snapshot that carries a non-null value. The merged rows MUST be returned sorted ascending by the chat span's `start_unix_ns` (treating null as 0).

## Rationale
A single chat span typically receives one `usage_info_event` snapshot (carrying `token_limit` + `current_tokens`) and one `chat_span` snapshot (carrying `input/output/reasoning/cache_read` tokens). The merge combines them into one row per turn while preferring the latest readings and tolerating either ordering of arrival.

## Derived from
- [[Context Growth Widget]]
