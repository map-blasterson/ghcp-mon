---
type: HLR
tags:
  - req/hlr
  - domain/vendor-adapter
  - vendor/copilot
---
Copilot CLI sessions carry workspace-metadata sidecar fields (`local_name`, `user_named`, `cwd`, `branch`) sourced from `~/.copilot/session-state/<cid>/workspace.yaml`. This HLR collects the Copilot-only UI specializations the Live Session Browser applies when `service_name === "github-copilot"` and those fields are available. The active adapter is selected per-span (per-session-row, here) by `service_name`.

## Derived LLRs
- [[Copilot session row workspace metadata]]
