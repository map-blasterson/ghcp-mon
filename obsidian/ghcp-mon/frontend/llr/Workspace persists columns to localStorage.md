---
type: LLR
tags:
  - req/llr
  - domain/workspace
---
The `useWorkspace` Zustand store MUST be wrapped with `persist` middleware whose storage key is `"ghcp-mon-workspace-v6"`, so the `columns`, `contextWidgetHeightVh`, and `contextWidgetVisible` fields survive a page reload.

## Rationale
The workspace is a personalised view; bumping the key suffix invalidates incompatible older snapshots.

## Derived from
- [[Workspace Layout]]
