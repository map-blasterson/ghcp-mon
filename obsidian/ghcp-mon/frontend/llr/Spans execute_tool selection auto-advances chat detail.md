---
type: LLR
tags:
  - req/llr
  - domain/traces
---
When the user picks a span whose `kind_class === "execute_tool"` in the spans column and a session span tree is loaded, `SpansScenario` MUST resolve a "following chat span" via shape-aware dispatch (`findFollowingChatSpanId`) and, for every `chat_detail` column whose allow-list excludes `execute_tool`, update that column's config with `selected_trace_id`, `selected_span_id` set to that chat span, and `selected_tool_call_id` set to `picked.projection.tool_call?.call_id`. Shape dispatch MUST follow these rules, evaluated in order: (1) **Nested shape** — if the picked tool span has any chat-class ancestor in the tree, choose the chat span anywhere in the tree whose `sortKey` (`end_unix_ns ?? start_unix_ns ?? span_pk ?? 0`, with `span_pk` tiebreaker) is the smallest value strictly greater than the nearest chat ancestor's sortKey; (2) **Sibling shape** — otherwise, choose the first chat sibling of the picked span (children of its parent) whose `[start_unix_ns, end_unix_ns]` interval temporally encloses the picked span's `[start, end]`; (3) **Fallback** — otherwise, choose the next chat sibling chronologically (sortKey strictly greater than the picked span's). When no chat span can be resolved by any rule, the chat_detail column's selection MUST be left unchanged. The dispatch MUST NOT consult `service_name` (vendor-agnostic).

## Rationale
OTel GenAI semconv permits two valid tool-span hierarchies, and the producers we observe use different ones. **Sibling-shape producers** dispatch tool spans as siblings of a still-running chat span and append the response to that same chat's input.messages — so the response-consuming chat is the sibling whose lifetime encloses the tool. **Nested-shape producers** nest tool spans under a chat span and surface the response in the *next* chat span globally (the one that fires after the parent chat completes and the loop iterates). Today's observed examples include Copilot CLI (sibling shape) and opencode (nested shape), but branching on tree shape rather than vendor name makes the routing robust to mixed traces.

## Derived from
- [[Trace and Span Explorer]]
- [[Span selection routes by kind class allow list]]
