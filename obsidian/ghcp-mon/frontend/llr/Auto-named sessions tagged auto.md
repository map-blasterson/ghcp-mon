---
type: LLR
tags:
  - req/llr
  - domain/live-sessions
---
A session row whose `local_name` is non-empty AND whose `user_named` is exactly `false` MUST render an `auto` badge with the title `"auto-summarized name (use /rename in copilot to set)"`.

## Rationale
Distinguishes Copilot's auto-generated name from a user-set one in the list.

## Derived from
- [[Live Session Browser]]
- [[API list sessions enriched with local workspace metadata]]
