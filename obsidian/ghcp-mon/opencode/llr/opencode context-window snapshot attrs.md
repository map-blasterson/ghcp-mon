---
type: LLR
tags:
  - req/llr
  - domain/vendor-adapter
  - vendor/opencode
  - domain/normalize
---
When a `Chat` span carries `gen_ai.opencode.context.token_limit` or `gen_ai.opencode.context.current_tokens`, the normalizer MUST populate the corresponding `context_snapshots` row (see [[Chat token usage attributes create context snapshot]]) with `token_limit` and `current_tokens` from those attributes, coalescing on conflict.

## Rationale
opencode reports context-window pressure under its own attribute namespace as an extension to the standard `gen_ai.usage.*` set.

## Test context
- [[Normalize Pipeline Cheatsheet]]

## Derived from
- [[opencode tool-call shape]]
- [[Chat token usage attributes create context snapshot]]

## Test case
- [[Normalize Pipeline Tests]]
