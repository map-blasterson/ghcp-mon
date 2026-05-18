---
type: LLR
tags:
  - req/llr
  - domain/traces
---
When the loaded session tree (`sessionTreeQ.data.tree`) updates, `SpansScenario` SHALL stage newly-arrived spans (any `span_id` not already revealed and not already queued) into a reveal queue and progressively unhide them so that downstream consumers (`displayedTree`, `nodeMap`, `latestToolSpan`, `SpanTreeView`, follow-mode) only see revealed spans. The reveal schedule MUST obey: (1) the very first non-empty batch on a fresh session — detected by both `revealedIds` and the existing queue being empty — reveals every fresh span immediately, with no animation; (2) subsequent batches map each fresh span's `start_unix_ns ?? end_unix_ns ?? 0` linearly across a 2000ms window so that `max(ts)` reveals at offset 0 and `min(ts)` at offset `SMOOTH_WINDOW_MS = 2000`ms (newest-first ordering); (3) a hierarchy clamp post-order walk MUST set every batched parent's reveal time to no later than the earliest reveal time of any of its batched descendants; (4) after merging into the queue, entries MUST be sorted ascending by reveal time and any entry within `SMOOTH_MIN_GAP_MS = 1000/60` ms of the previous entry MUST be delayed so consecutive reveals are at least that gap apart (≤ 60 reveals/sec global cap); (5) switching `session` MUST synchronously clear `revealedIds`, the queue, and any pending timer, and trigger a re-render so the new session starts as a fresh first-load.

## Rationale
Spans arrive in bursts taller than the viewport. Revealing each batch atomically destroys the perceptual ingestion rate. Linearly mapping arrival times to a 2-second window, in newest-first order to match the list orientation, and clamping the global reveal rate to 60/sec, produces a visible cadence proportional to the source rate without ever stalling the UI. The hierarchy clamp prevents the filter from hiding a child whose parent reveals later, which would otherwise collapse the cadence into a simultaneous appearance.

## Derived from
- [[Trace and Span Explorer]]
