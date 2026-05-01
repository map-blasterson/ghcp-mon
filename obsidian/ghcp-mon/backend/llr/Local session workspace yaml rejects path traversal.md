---
type: LLR
tags:
  - req/llr
  - domain/local-session
---
`local_session::read_workspace_yaml(base, cid)` MUST return `None` without touching the filesystem when `cid` is empty or contains any of the substrings `/`, `\\`, or `..`.

## Rationale
Defense-in-depth: `cid` arrives from URL paths, so the lookup must never escape `base` even if a malicious caller supplies traversal segments.

## Test context
- [[Local Session Cheatsheet]]

## Derived from
- [[Local Session Metadata]]

## Test case
- [[Local Session Tests]]
