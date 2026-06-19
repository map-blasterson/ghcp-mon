---
type: LLR
tags:
  - req/llr
  - domain/input-breakdown
---
When `mode === "DELTA"` and a `prior` chat span is available with a non-empty `prior.inputMessages`, `buildInputMessagesNode` SHALL detect a cumulative-input shape by computing `prefixLen = commonPrefixLen(current, prior.inputMessages)` (the length of the longest prefix whose elements deep-equal the corresponding entries by JSON serialization); when `prefixLen === prior.inputMessages.length` AND `current.length > prefixLen`, the rendered `input_messages` subtree MUST contain a single `input_messages_unchanged` child labeled `"carried forward"` (with meta `"unchanged · {prefixLen} message(s) from prior turn"`) followed by message nodes for only the suffix `current.slice(prefixLen)`, where each suffix node's id preserves the original-array index (`root/input/input_messages/{prefixLen + j}`). When the strict-prefix condition does not hold, every message MUST be rendered with full meta `"{N} message(s) · per-turn delta"`.

## Rationale
Producers that emit cumulative input history on every chat span re-stamp the entire conversation each turn; the suffix optimization elides the carried-forward prefix. Producers that emit per-turn input deltas share no message ids with the prior chat, so the optimization correctly no-ops. Preserving the original-array index in suffix node ids keeps external cross-links (e.g., ChatDetail's tool-call arrow targeting a `tool_call_response`) stable regardless of whether the optimization fires.

## Derived from
- [[Chat detail DELTA diffs against prior chat span]]
- [[Chat detail]]
