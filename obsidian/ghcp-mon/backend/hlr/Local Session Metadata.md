---
type: HLR
tags:
  - req/hlr
  - domain/local-session
---
The dashboard surfaces human-readable session metadata (display name, working directory, git branch, user-renamed flag) by reading the sidecar `workspace.yaml` files that the GitHub Copilot CLI writes alongside each conversation. This metadata is not carried over OTel and is read best-effort: if the file is missing or unparseable, the dashboard simply omits it.

## Derived from
- [[Source Index]]
