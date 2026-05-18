---
type: LLR
tags:
  - req/llr
  - domain/context-growth
---
For every chat bar in the Context Growth Widget chart (root agent and sub-agent alike), the renderer MUST split the merged `input_tokens` into a `cache_read` portion `cacheR = min(cache_read_tokens ?? 0, input_tokens ?? 0)` and a fresh portion `inp = max(0, (input_tokens ?? 0) - cacheR)`, then stack the sub-bars from the baseline upward in this fixed order: cache-read (green `#4ade80`), fresh input (blue `#60a5fa` for root, `#93c5fd` for sub-agent), output (orange `#fb923c`), reasoning (yellow `#fde047`). The bar's overall height MUST be proportional to `total = cacheR + inp + out + rea` (not to the raw input). When `total === 0` no sub-bars are rendered. The hover `title` tooltip MUST disclose `cache_read`, `input` with the raw value (`input=<inp> (raw=<rawInp>)`), `output`, `reasoning`, `total`, and `limit`.

## Rationale
Surfacing cache-read tokens as a green segment makes it visually obvious how much of each turn's prompt was served from the provider's prompt cache versus newly billed input. Clamping `cacheR ≤ rawInp` defends against rare snapshots where `cache_read_tokens > input_tokens`, and stacking cache-read at the baseline keeps the fresh-input blue segment in a consistent position across turns.

## Derived from
- [[Context Growth Widget]]
