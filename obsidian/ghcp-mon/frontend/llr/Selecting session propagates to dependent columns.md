---
type: LLR
tags:
  - req/llr
  - domain/live-sessions
---
When the user clicks a session row, `LiveSessionsScenario` MUST set `config.session = conversation_id` on its own column AND on every other column whose `scenarioType` is one of `"spans"`, `"input_breakdown"`, `"file_touches"`, leaving the other columns' `config` fields otherwise unchanged.

## Rationale
The dependent scenarios are session-scoped; selection acts as a cross-column "switch session" command.

## Derived from
- [[Live Session Browser]]
