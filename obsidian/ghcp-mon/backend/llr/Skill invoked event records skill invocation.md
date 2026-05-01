---
type: LLR
tags:
  - req/llr
  - domain/normalize
---
For each span event named `github.copilot.skill.invoked`, the normalizer MUST insert a `skill_invocations` row carrying `span_pk`, `skill_name`, `skill_path`, `invoked_unix_ns`, and `conversation_id`, doing nothing on conflict against the unique key `(span_pk, invoked_unix_ns, skill_name)`.

## Rationale
Idempotency under re-delivery: replaying the same span must not duplicate skill invocation rows.

## Test context
- [[Normalize Pipeline Cheatsheet]]

## Derived from
- [[Span Normalization]]

## Test case
- [[Normalize Pipeline Tests]]
