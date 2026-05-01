---
type: LLR
tags:
  - req/llr
  - domain/file-touches
---
`FileTouchesScenario` MUST subscribe to `useLiveFeed([{ kind: "span", entity: "span" }, { kind: "derived", entity: "tool_call" }])` and, on each `tick`, MUST invalidate the session-scoped `["spans", { session, kind: "execute_tool", limit: 1000 }]` query.

## Rationale
New tool calls and span upgrades must surface in the file tree without a manual refresh.

## Derived from
- [[File Touch Tree]]
