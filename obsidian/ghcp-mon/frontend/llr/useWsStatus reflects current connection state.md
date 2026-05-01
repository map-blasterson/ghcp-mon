---
type: LLR
tags:
  - req/llr
  - domain/live-events
---
The `useWsStatus()` hook MUST return a boolean equal to `wsBus.isConnected()` on first render, MUST start the bus if not already running, and MUST cause the consuming component to re-render whenever the bus's connection status changes.

## Rationale
The top-bar status dot needs to reflect connection state without prop-drilling.

## Derived from
- [[Live WebSocket Subscription]]
