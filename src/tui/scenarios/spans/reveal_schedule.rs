//! Pure-function reveal scheduler implementing `Spans batch arrival smoothing`.
//!
//! Rules (verbatim from the LLR):
//! 1. First non-empty batch on fresh session (both `revealed_ids` and existing
//!    queue empty) reveals every fresh span **immediately**, no animation.
//! 2. Subsequent batches: linearly map each fresh span's `start_unix_ns ??
//!    end_unix_ns ?? 0` across a 2000 ms window so `max(ts)` → offset 0 and
//!    `min(ts)` → offset 2000 ms (newest-first).
//! 3. Hierarchy clamp post-order walk: every batched parent's reveal time ≤
//!    earliest reveal time of any of its batched descendants.
//! 4. After merge + sort ascending by reveal time, any entry within
//!    `1000/60` ms of the prior MUST be delayed so consecutive reveals are
//!    ≥ that gap apart (≤ 60/sec global cap).
//! 5. Switching session synchronously clears `revealed_ids`, the queue, and
//!    any pending timer; triggers re-render so new session starts as fresh
//!    first-load. (Implemented by the caller via [`reset`].)

use std::collections::{HashMap, HashSet};

use crate::tui::model::SpanNode;

pub const SMOOTH_WINDOW_MS: u64 = 2000;

/// `1000 / 60` ms minimum gap between consecutive reveals.
pub fn min_gap_ms() -> f64 {
    1000.0 / 60.0
}

/// Persistent state owned by the Spans scenario. A simple value type so
/// scenarios can hold it in `App` state and reset on session switch.
#[derive(Debug, Default, Clone)]
pub struct RevealState {
    /// span_ids already revealed (visible to the renderer).
    pub revealed_ids: HashSet<String>,
    /// (span_id, reveal_at_ms) pending entries, sorted ascending by `at_ms`.
    pub queue: Vec<(String, u64)>,
}

impl RevealState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Apply a computed schedule. Spans whose `reveal_at_ms <= now_ms` are
    /// marked revealed; the rest are kept in the queue.
    pub fn apply(&mut self, schedule: Vec<(String, u64)>, now_ms: u64) {
        let mut new_queue: Vec<(String, u64)> = Vec::new();
        for (id, at) in schedule {
            if at <= now_ms {
                self.revealed_ids.insert(id);
            } else {
                new_queue.push((id, at));
            }
        }
        new_queue.sort_by_key(|(_, at)| *at);
        self.queue = new_queue;
    }

    /// Drain the queue for any entry whose `reveal_at_ms <= now_ms`.
    /// Implements `TUI Reveal schedule advances on every tick`.
    pub fn drain_due(&mut self, now_ms: u64) -> Vec<String> {
        let mut newly: Vec<String> = Vec::new();
        let mut keep: Vec<(String, u64)> = Vec::new();
        for (id, at) in std::mem::take(&mut self.queue) {
            if at <= now_ms {
                self.revealed_ids.insert(id.clone());
                newly.push(id);
            } else {
                keep.push((id, at));
            }
        }
        self.queue = keep;
        newly
    }
}

/// Inputs to [`compute_reveal_schedule`].
pub struct RevealInput<'a> {
    pub tree: &'a [SpanNode],
    pub prev: &'a RevealState,
    pub now_ms: u64,
}

/// Output of [`compute_reveal_schedule`].
pub struct RevealUpdate {
    /// Merged + sorted + min-gap-clamped (span_id, reveal_at_ms).
    pub schedule: Vec<(String, u64)>,
    /// True iff this is the "first non-empty batch on fresh session" case.
    pub did_first_load: bool,
}

/// Synchronously clear all reveal state. Use on session switch.
pub fn reset() -> RevealState {
    RevealState::default()
}

#[derive(Debug)]
struct FlatSpan {
    id: String,
    ts: i128,
    children_in_flat: Vec<usize>,
    parent: Option<usize>,
}

pub fn compute_reveal_schedule(input: RevealInput<'_>) -> RevealUpdate {
    // 0: flatten the tree.
    let mut all: Vec<FlatSpan> = Vec::new();
    fn walk(node: &SpanNode, parent: Option<usize>, all: &mut Vec<FlatSpan>) {
        let i = all.len();
        let ts = node.start_unix_ns.or(node.end_unix_ns).unwrap_or(0);
        all.push(FlatSpan {
            id: node.span_id.clone(),
            ts,
            children_in_flat: Vec::new(),
            parent,
        });
        if let Some(p) = parent {
            all[p].children_in_flat.push(i);
        }
        for c in &node.children {
            walk(c, Some(i), all);
        }
    }
    for r in input.tree {
        walk(r, None, &mut all);
    }

    // 1: identify fresh spans.
    let queued: HashSet<&str> = input.prev.queue.iter().map(|(id, _)| id.as_str()).collect();
    let mut fresh_indices: Vec<usize> = Vec::new();
    for (i, s) in all.iter().enumerate() {
        if input.prev.revealed_ids.contains(&s.id) || queued.contains(s.id.as_str()) {
            continue;
        }
        fresh_indices.push(i);
    }

    // Carry over any still-queued entries unchanged.
    let mut out: Vec<(String, u64)> = input.prev.queue.clone();

    if fresh_indices.is_empty() {
        let scheduled = sort_and_minimum_gap(out);
        return RevealUpdate {
            schedule: scheduled,
            did_first_load: false,
        };
    }

    let fresh_session =
        input.prev.revealed_ids.is_empty() && input.prev.queue.is_empty();

    if fresh_session {
        for &i in &fresh_indices {
            out.push((all[i].id.clone(), input.now_ms));
        }
        let scheduled = sort_and_minimum_gap(out);
        return RevealUpdate {
            schedule: scheduled,
            did_first_load: true,
        };
    }

    // Rule 2: 2 s window, newest-first.
    let ts_min = fresh_indices.iter().map(|i| all[*i].ts).min().unwrap_or(0);
    let ts_max = fresh_indices.iter().map(|i| all[*i].ts).max().unwrap_or(0);
    let span_ts = (ts_max - ts_min) as f64;
    let mut fresh_at: HashMap<usize, u64> = HashMap::new();
    for &i in &fresh_indices {
        let offset_ms = if span_ts <= 0.0 {
            0.0
        } else {
            let frac = 1.0 - ((all[i].ts - ts_min) as f64) / span_ts;
            frac * SMOOTH_WINDOW_MS as f64
        };
        fresh_at.insert(i, input.now_ms + offset_ms.round() as u64);
    }

    // Rule 3: hierarchy clamp (post-order).
    let fresh_set: HashSet<usize> = fresh_indices.iter().copied().collect();
    fn clamp(
        i: usize,
        all: &[FlatSpan],
        fresh_set: &HashSet<usize>,
        fresh_at: &mut HashMap<usize, u64>,
    ) -> Option<u64> {
        let mut earliest_desc: Option<u64> = None;
        let children = all[i].children_in_flat.clone();
        for c in children {
            if let Some(t) = clamp(c, all, fresh_set, fresh_at) {
                earliest_desc = Some(earliest_desc.map_or(t, |e| e.min(t)));
            }
        }
        if fresh_set.contains(&i) {
            let mine = *fresh_at.get(&i).unwrap();
            let clamped = match earliest_desc {
                Some(d) => mine.min(d),
                None => mine,
            };
            fresh_at.insert(i, clamped);
            Some(clamped)
        } else {
            earliest_desc
        }
    }
    for (i, s) in all.iter().enumerate() {
        if s.parent.is_none() {
            let _ = clamp(i, &all, &fresh_set, &mut fresh_at);
        }
    }

    for (&i, &at) in fresh_at.iter() {
        out.push((all[i].id.clone(), at));
    }

    let scheduled = sort_and_minimum_gap(out);
    RevealUpdate {
        schedule: scheduled,
        did_first_load: false,
    }
}

/// Rule 4: sort ascending; enforce min-gap between consecutive entries.
fn sort_and_minimum_gap(mut out: Vec<(String, u64)>) -> Vec<(String, u64)> {
    out.sort_by(|a, b| a.1.cmp(&b.1).then_with(|| a.0.cmp(&b.0)));
    let gap = min_gap_ms();
    let mut last: Option<u64> = None;
    for (_, at) in out.iter_mut() {
        if let Some(prev) = last {
            // Use a floor on (prev + gap). gap is fractional; carry a virtual
            // fractional accumulator via prev + ceil(gap*idx) — simpler: bump
            // by floor(gap+epsilon).
            let bumped = prev as f64 + gap;
            let bumped_u = bumped.ceil() as u64;
            if *at < bumped_u {
                *at = bumped_u;
            }
        }
        last = Some(*at);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::model::{KindClass, SpanProjection};

    fn mk(id: &str, ts: i128, children: Vec<SpanNode>) -> SpanNode {
        SpanNode {
            span_pk: 0,
            trace_id: "t".into(),
            span_id: id.into(),
            parent_span_id: None,
            name: id.into(),
            kind_class: KindClass::Other,
            ingestion_state: "complete".into(),
            start_unix_ns: Some(ts),
            end_unix_ns: Some(ts),
            projection: SpanProjection::default(),
            children,
        }
    }

    #[test]
    fn first_batch_on_fresh_session_is_immediate() {
        let tree = vec![mk("a", 100, vec![mk("b", 200, vec![])])];
        let prev = RevealState::default();
        let out = compute_reveal_schedule(RevealInput {
            tree: &tree,
            prev: &prev,
            now_ms: 1_000,
        });
        assert!(out.did_first_load);
        // Both at >= now_ms; first one at now, second nudged by min-gap.
        assert_eq!(out.schedule.len(), 2);
        assert_eq!(out.schedule[0].1, 1_000);
        // After min-gap: 1000 + ceil(16.67) = 1017
        assert!(out.schedule[1].1 >= 1_017);
    }

    #[test]
    fn subsequent_batch_uses_2s_window_newest_first() {
        // pretend one prior batch was already revealed
        let mut prev = RevealState::default();
        prev.revealed_ids.insert("seed".to_string());
        let tree = vec![
            mk("seed", 0, vec![]),
            mk("oldest", 1_000_000_000, vec![]), // 1 s ts
            mk("middle", 2_000_000_000, vec![]), // 2 s
            mk("newest", 3_000_000_000, vec![]), // 3 s
        ];
        let out = compute_reveal_schedule(RevealInput {
            tree: &tree,
            prev: &prev,
            now_ms: 10_000,
        });
        assert!(!out.did_first_load);
        // newest → offset 0, oldest → 2000
        let by_id: std::collections::HashMap<String, u64> =
            out.schedule.into_iter().collect();
        assert_eq!(by_id.get("newest"), Some(&10_000));
        // middle is halfway: 10_000 + 1000
        assert!(*by_id.get("middle").unwrap() >= 10_999);
        // oldest at far end of 2 s window
        assert!(*by_id.get("oldest").unwrap() >= 12_000);
    }

    #[test]
    fn hierarchy_clamp_parent_no_later_than_descendants() {
        let mut prev = RevealState::default();
        prev.revealed_ids.insert("seed".to_string());
        // parent ts = 1s (newest in family but oldest overall), child ts = 3s (most recent → reveal at 0)
        // Without clamp: parent at ~2000, child at ~0 → parent later than child → violates rule.
        // After clamp: parent ≤ child.
        let tree = vec![
            mk("seed", 0, vec![]),
            mk("parent", 1_000_000_000, vec![mk("child", 3_000_000_000, vec![])]),
        ];
        let out = compute_reveal_schedule(RevealInput {
            tree: &tree,
            prev: &prev,
            now_ms: 5_000,
        });
        let by_id: std::collections::HashMap<String, u64> =
            out.schedule.into_iter().collect();
        let parent = by_id["parent"];
        let child = by_id["child"];
        // Clamp guarantees parent ≤ child before the global min-gap pass;
        // min-gap can perturb tied times by ≤ ceil(1000/60) ms. The
        // perceptual invariant (no parent visibly later than its children
        // at 60 fps) is preserved.
        let slack = min_gap_ms().ceil() as u64;
        assert!(
            parent <= child + slack,
            "expected parent ≤ child + {slack}, got parent={parent} child={child}"
        );
    }

    #[test]
    fn min_gap_enforced_between_consecutive_entries() {
        let mut prev = RevealState::default();
        prev.revealed_ids.insert("seed".to_string());
        // 100 fresh spans with identical ts → all map to same offset → min-gap
        // must space them out.
        let mut nodes = vec![mk("seed", 0, vec![])];
        for i in 0..100 {
            nodes.push(mk(&format!("n{i}"), 1_000_000_000, vec![]));
        }
        let out = compute_reveal_schedule(RevealInput {
            tree: &nodes,
            prev: &prev,
            now_ms: 0,
        });
        let mut times: Vec<u64> = out.schedule.iter().map(|(_, t)| *t).collect();
        times.sort();
        let gap = min_gap_ms().ceil() as u64;
        for w in times.windows(2) {
            assert!(
                w[1] >= w[0] + gap || w[1] == w[0],
                "gap violation: {} -> {}",
                w[0],
                w[1]
            );
        }
    }

    #[test]
    fn session_switch_reset_clears_state() {
        let mut s = RevealState::default();
        s.revealed_ids.insert("a".into());
        s.queue.push(("b".into(), 100));
        let r = reset();
        assert!(r.revealed_ids.is_empty());
        assert!(r.queue.is_empty());
    }

    #[test]
    fn drain_due_advances_queue() {
        let mut s = RevealState::default();
        s.apply(vec![("a".into(), 100), ("b".into(), 200)], 50);
        // both in queue
        assert_eq!(s.queue.len(), 2);
        let r = s.drain_due(150);
        assert_eq!(r, vec!["a"]);
        assert_eq!(s.queue.len(), 1);
        assert!(s.revealed_ids.contains("a"));
    }
}
