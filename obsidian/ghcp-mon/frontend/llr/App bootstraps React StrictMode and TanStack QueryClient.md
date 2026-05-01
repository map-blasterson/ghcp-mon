---
type: LLR
tags:
  - req/llr
  - domain/workspace
---
`web/src/main.tsx` MUST mount the `<App />` tree under `React.StrictMode` wrapped in a `QueryClientProvider`, into the DOM element with id `root`.

## Rationale
StrictMode catches dev-time anti-patterns; QueryClientProvider is the root for every TanStack Query in the app.

## Derived from
- [[Workspace Layout]]
