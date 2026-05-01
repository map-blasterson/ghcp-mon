---
type: LLR
tags:
  - req/llr
  - domain/input-breakdown
---
The breakdown tree's root MUST contain exactly four child branches: `system instructions` (parsed from `gen_ai.system_instructions`), `tool definitions` (parsed from `gen_ai.tool.definitions`), `input messages` (from `gen_ai.input.messages`), and `output messages` (from `gen_ai.output.messages`).

## Rationale
These are the four content attributes defined by the OTel GenAI semantic-convention spec.

## Derived from
- [[Chat Input Breakdown]]
