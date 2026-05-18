---
type: LLR
tags:
  - req/llr
  - domain/live-sessions
  - github-specific
---
A session row whose `service_name` is `"github-copilot"` AND whose `local_name` is non-empty AND whose `user_named` is exactly `false` MUST render an `auto` badge with the title `"auto-summarized name (use /rename in copilot to set)"`. For sessions from other service names the badge MUST NOT render even if `user_named === false` (see [[Sessions service name gates vendor UI]]).

## Rationale
Distinguishes Copilot's auto-generated name from a user-set one in the list.

## Derived from
- [[Live Session Browser]]
- [[API list sessions enriched with local workspace metadata]]
