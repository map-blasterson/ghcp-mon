---
type: HLR
tags:
  - req/hlr
  - domain/workspace
---
The dashboard presents the user with a configurable workspace of side-by-side scenario columns whose ordering, widths, titles, and presence are persisted across reloads, plus a top-bar control surface for adding, resetting, and observing the live connection status.

## Derived LLRs
- [[App bootstraps React StrictMode and TanStack QueryClient]]
- [[Default TanStack Query options no refetch on focus]]
- [[Top bar adds columns selects scenario type]]
- [[Top bar exposes ws connection indicator]]
- [[Workspace persists columns to localStorage]]
- [[Workspace migration drops obsolete scenario types]]
- [[Default workspace seeds four columns]]
- [[Workspace lays out columns by weight with ResizeObserver]]
- [[Workspace minimum column width 120 px]]
- [[Resizer drag rebalances two adjacent column weights]]
- [[Empty workspace shows empty state]]
- [[Column body dispatches by scenario type]]
- [[Column header allows rename move remove]]
- [[Scenario registry maps scenario type to component]]
