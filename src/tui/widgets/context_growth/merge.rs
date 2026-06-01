//! Pure snapshot-merge logic for the Context Growth Widget.
//!
//! Implements `Context widget merges chat span snapshots per span_pk` and the
//! sub-agent inclusion / flagging rules from `Context widget includes
//! sub-agent chats` + `Context widget colors sub-agent input bar distinctly`.
//!
//! This module is deliberately free of any rendering concern so the merge
//! rules can be unit-tested exhaustively.

use std::collections::{HashMap, HashSet};

use crate::tui::model::{ContextSnapshot, KindClass, SpanNode};

/// One merged chart row — a single chat turn's aggregated token usage.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MergedRow {
    pub span_pk: i64,
    pub token_limit: Option<i64>,
    pub input_tokens: Option<i64>,
    pub output_tokens: Option<i64>,
    pub reasoning_tokens: Option<i64>,
    pub cache_read_tokens: Option<i64>,
    /// Largest `captured_ns` observed across the span's snapshots.
    pub latest_ns: i64,
    /// True when the chat span's `invoke_agent` ancestor depth is > 1
    /// (i.e., it belongs to a sub-agent rather than the root agent).
    pub is_sub_agent: bool,
}

/// Per-chat-span structural info derived from the span tree.
#[derive(Debug, Clone, Copy)]
struct ChatInfo {
    /// `start_unix_ns` (null treated as 0) — drives row sort order.
    start_ns: i64,
    /// Number of `invoke_agent` ancestors above this chat span.
    invoke_depth: usize,
}

/// Walk the span tree and collect `span_pk -> ChatInfo` for every
/// `kind_class == chat` span, tracking the count of `invoke_agent` ancestors.
fn build_chat_infos(tree: &[SpanNode]) -> HashMap<i64, ChatInfo> {
    fn walk(node: &SpanNode, invoke_count: usize, out: &mut HashMap<i64, ChatInfo>) {
        if matches!(node.kind_class, KindClass::Chat) {
            out.insert(
                node.span_pk,
                ChatInfo {
                    start_ns: node.start_unix_ns.unwrap_or(0) as i64,
                    invoke_depth: invoke_count,
                },
            );
        }
        let child_invoke =
            invoke_count + usize::from(matches!(node.kind_class, KindClass::InvokeAgent));
        for c in &node.children {
            walk(c, child_invoke, out);
        }
    }
    let mut out = HashMap::new();
    for r in tree {
        walk(r, 0, &mut out);
    }
    out
}

/// Build the set of chat-span `span_pk`s present in the tree. The renderer
/// passes this set to [`merge_snapshots`] as the membership filter.
pub fn chat_span_pks(tree: &[SpanNode]) -> HashSet<i64> {
    build_chat_infos(tree).into_keys().collect()
}

/// Maximum `current_tokens` observed across snapshots (the y-axis occupancy
/// anchor per `Context widget stack chart per turn`). Snapshots without a
/// `current_tokens` value do not contribute.
pub fn max_current_tokens(snapshots: &[ContextSnapshot]) -> i64 {
    snapshots
        .iter()
        .filter_map(|s| s.current_tokens)
        .max()
        .unwrap_or(0)
}

/// Per-field non-null accumulator that prefers the value carried by the
/// snapshot with the largest `captured_ns` and back-fills from earlier
/// snapshots when later ones are null.
#[derive(Default, Clone, Copy)]
struct LatestNonNull {
    value: Option<i64>,
    at_ns: i64,
}

impl LatestNonNull {
    fn observe(&mut self, v: Option<i64>, captured_ns: i64) {
        if let Some(v) = v {
            if self.value.is_none() || captured_ns >= self.at_ns {
                self.value = Some(v);
                self.at_ns = captured_ns;
            }
        }
    }
}

/// Merge per-`span_pk` `context_snapshots` into one [`MergedRow`] per chat
/// turn. See `Context widget merges chat span snapshots per span_pk` for the
/// verbatim rules:
///
/// - (a) Ignore snapshots whose `span_pk` is null or not in `chat_span_pks`.
/// - (b) `token_limit` = max non-null observed across the span's snapshots.
/// - (c) For input/output/reasoning/cache_read: prefer the value from the
///   snapshot with the largest `captured_ns` (overwriting only when
///   non-null), otherwise back-fill any field still null from any earlier
///   snapshot carrying a non-null value.
/// - Rows are returned sorted ascending by the chat span's `start_unix_ns`
///   (null treated as 0), ties broken by `span_pk` for determinism.
pub fn merge_snapshots(
    chat_span_pks: &HashSet<i64>,
    snapshots: &[ContextSnapshot],
    tree: &[SpanNode],
) -> Vec<MergedRow> {
    let infos = build_chat_infos(tree);

    struct Acc {
        token_limit: Option<i64>,
        input: LatestNonNull,
        output: LatestNonNull,
        reasoning: LatestNonNull,
        cache_read: LatestNonNull,
        latest_ns: i64,
    }
    let mut accs: HashMap<i64, Acc> = HashMap::new();

    for snap in snapshots {
        let Some(span_pk) = snap.span_pk else {
            continue; // (a) null span_pk
        };
        if !chat_span_pks.contains(&span_pk) {
            continue; // (a) not a chat span
        }
        let captured = snap.captured_ns as i64;
        let acc = accs.entry(span_pk).or_insert_with(|| Acc {
            token_limit: None,
            input: LatestNonNull::default(),
            output: LatestNonNull::default(),
            reasoning: LatestNonNull::default(),
            cache_read: LatestNonNull::default(),
            latest_ns: i64::MIN,
        });
        // (b) token_limit = max non-null.
        if let Some(tl) = snap.token_limit {
            acc.token_limit = Some(acc.token_limit.map_or(tl, |cur| cur.max(tl)));
        }
        // (c) latest-non-null per field.
        acc.input.observe(snap.input_tokens, captured);
        acc.output.observe(snap.output_tokens, captured);
        acc.reasoning.observe(snap.reasoning_tokens, captured);
        acc.cache_read.observe(snap.cache_read_tokens, captured);
        acc.latest_ns = acc.latest_ns.max(captured);
    }

    let mut rows: Vec<MergedRow> = accs
        .into_iter()
        .map(|(span_pk, acc)| {
            let info = infos.get(&span_pk).copied();
            MergedRow {
                span_pk,
                token_limit: acc.token_limit,
                input_tokens: acc.input.value,
                output_tokens: acc.output.value,
                reasoning_tokens: acc.reasoning.value,
                cache_read_tokens: acc.cache_read.value,
                latest_ns: if acc.latest_ns == i64::MIN { 0 } else { acc.latest_ns },
                is_sub_agent: info.map(|i| i.invoke_depth > 1).unwrap_or(false),
            }
        })
        .collect();

    rows.sort_by(|a, b| {
        let sa = infos.get(&a.span_pk).map(|i| i.start_ns).unwrap_or(0);
        let sb = infos.get(&b.span_pk).map(|i| i.start_ns).unwrap_or(0);
        sa.cmp(&sb).then(a.span_pk.cmp(&b.span_pk))
    });
    rows
}

/// Split a row's `input_tokens` into the cache-read portion and the fresh
/// portion per `Context widget cache read green segment`:
/// `cacheR = min(cache_read ?? 0, input ?? 0)`, `inp = max(0, input - cacheR)`.
/// Returns `(cache_read_clamped, fresh_input, output, reasoning)`.
pub fn split_segments(row: &MergedRow) -> (i64, i64, i64, i64) {
    let input = row.input_tokens.unwrap_or(0).max(0);
    let cache = row.cache_read_tokens.unwrap_or(0).max(0);
    let cache_r = cache.min(input);
    let fresh = (input - cache_r).max(0);
    let out = row.output_tokens.unwrap_or(0).max(0);
    let rea = row.reasoning_tokens.unwrap_or(0).max(0);
    (cache_r, fresh, out, rea)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::model::SpanProjection;

    #[allow(clippy::too_many_arguments)]
    fn snap(
        span_pk: Option<i64>,
        captured_ns: i128,
        token_limit: Option<i64>,
        current: Option<i64>,
        input: Option<i64>,
        output: Option<i64>,
        reasoning: Option<i64>,
        cache_read: Option<i64>,
    ) -> ContextSnapshot {
        ContextSnapshot {
            ctx_pk: 0,
            span_pk,
            captured_ns,
            token_limit,
            current_tokens: current,
            messages_length: None,
            input_tokens: input,
            output_tokens: output,
            cache_read_tokens: cache_read,
            reasoning_tokens: reasoning,
            source: None,
        }
    }

    fn chat(span_pk: i64, start: i128, children: Vec<SpanNode>) -> SpanNode {
        node(span_pk, KindClass::Chat, start, children)
    }

    fn node(span_pk: i64, kind: KindClass, start: i128, children: Vec<SpanNode>) -> SpanNode {
        SpanNode {
            span_pk,
            trace_id: "t".into(),
            span_id: format!("s{span_pk}"),
            parent_span_id: None,
            name: format!("n{span_pk}"),
            kind_class: kind,
            ingestion_state: "complete".into(),
            start_unix_ns: Some(start),
            end_unix_ns: Some(start),
            projection: SpanProjection::default(),
            children,
        }
    }

    #[test]
    fn empty_snapshots_yields_no_rows() {
        let tree = vec![chat(1, 0, vec![])];
        let pks = chat_span_pks(&tree);
        assert!(merge_snapshots(&pks, &[], &tree).is_empty());
    }

    #[test]
    fn single_snapshot_with_all_fields() {
        let tree = vec![chat(1, 10, vec![])];
        let pks = chat_span_pks(&tree);
        let snaps = vec![snap(
            Some(1),
            100,
            Some(8000),
            Some(5000),
            Some(4000),
            Some(120),
            Some(50),
            Some(1000),
        )];
        let rows = merge_snapshots(&pks, &snaps, &tree);
        assert_eq!(rows.len(), 1);
        let r = &rows[0];
        assert_eq!(r.span_pk, 1);
        assert_eq!(r.token_limit, Some(8000));
        assert_eq!(r.input_tokens, Some(4000));
        assert_eq!(r.output_tokens, Some(120));
        assert_eq!(r.reasoning_tokens, Some(50));
        assert_eq!(r.cache_read_tokens, Some(1000));
        assert_eq!(r.latest_ns, 100);
        assert!(!r.is_sub_agent);
    }

    #[test]
    fn two_snapshots_backfill_nulls() {
        // usage_info_event: token_limit + current only.
        // chat_span: input/output/reasoning/cache_read only.
        let tree = vec![chat(1, 10, vec![])];
        let pks = chat_span_pks(&tree);
        let usage = snap(Some(1), 100, Some(8000), Some(5000), None, None, None, None);
        let chat_span = snap(
            Some(1),
            90,
            None,
            None,
            Some(4000),
            Some(120),
            Some(50),
            Some(1000),
        );
        let rows = merge_snapshots(&pks, &[usage, chat_span], &tree);
        assert_eq!(rows.len(), 1);
        let r = &rows[0];
        // token_limit comes from usage (later), token fields backfilled from earlier chat_span.
        assert_eq!(r.token_limit, Some(8000));
        assert_eq!(r.input_tokens, Some(4000));
        assert_eq!(r.output_tokens, Some(120));
        assert_eq!(r.cache_read_tokens, Some(1000));
        // latest_ns is the max captured_ns.
        assert_eq!(r.latest_ns, 100);
    }

    #[test]
    fn latest_non_null_wins_when_both_present() {
        let tree = vec![chat(1, 10, vec![])];
        let pks = chat_span_pks(&tree);
        let old = snap(Some(1), 50, None, None, Some(100), None, None, None);
        let new = snap(Some(1), 200, None, None, Some(999), None, None, None);
        let rows = merge_snapshots(&pks, &[old, new], &tree);
        assert_eq!(rows[0].input_tokens, Some(999));
        assert_eq!(rows[0].latest_ns, 200);
    }

    #[test]
    fn token_limit_takes_max_non_null() {
        let tree = vec![chat(1, 10, vec![])];
        let pks = chat_span_pks(&tree);
        let a = snap(Some(1), 50, Some(4000), None, None, None, None, None);
        let b = snap(Some(1), 60, Some(8000), None, None, None, None, None);
        let c = snap(Some(1), 70, None, None, None, None, None, None);
        let rows = merge_snapshots(&pks, &[a, b, c], &tree);
        assert_eq!(rows[0].token_limit, Some(8000));
    }

    #[test]
    fn skips_null_span_pk() {
        let tree = vec![chat(1, 10, vec![])];
        let pks = chat_span_pks(&tree);
        let s = snap(None, 100, Some(1), Some(1), Some(1), None, None, None);
        assert!(merge_snapshots(&pks, &[s], &tree).is_empty());
    }

    #[test]
    fn skips_span_pk_not_in_chat_set() {
        let tree = vec![chat(1, 10, vec![])];
        let pks = chat_span_pks(&tree);
        // span_pk 99 is not a chat span.
        let s = snap(Some(99), 100, Some(1), Some(1), Some(1), None, None, None);
        assert!(merge_snapshots(&pks, &[s], &tree).is_empty());
    }

    #[test]
    fn rows_sorted_by_start_ns_null_first() {
        // chat 2 has null start (treated as 0), chat 1 start=500, chat 3 start=100.
        let mut c2 = chat(2, 0, vec![]);
        c2.start_unix_ns = None;
        let tree = vec![chat(1, 500, vec![]), c2, chat(3, 100, vec![])];
        let pks = chat_span_pks(&tree);
        let snaps = vec![
            snap(Some(1), 1, None, None, Some(1), None, None, None),
            snap(Some(2), 1, None, None, Some(1), None, None, None),
            snap(Some(3), 1, None, None, Some(1), None, None, None),
        ];
        let rows = merge_snapshots(&pks, &snaps, &tree);
        let order: Vec<i64> = rows.iter().map(|r| r.span_pk).collect();
        // null(0) -> 2, then 100 -> 3, then 500 -> 1.
        assert_eq!(order, vec![2, 3, 1]);
    }

    #[test]
    fn sub_agent_depth_detection() {
        // root invoke_agent -> chat (depth 1, root) and nested invoke_agent -> chat (depth 2, sub).
        let tree = vec![node(
            10,
            KindClass::InvokeAgent,
            0,
            vec![
                chat(1, 10, vec![]),
                node(20, KindClass::InvokeAgent, 5, vec![chat(2, 20, vec![])]),
            ],
        )];
        let pks = chat_span_pks(&tree);
        let snaps = vec![
            snap(Some(1), 1, None, None, Some(1), None, None, None),
            snap(Some(2), 1, None, None, Some(1), None, None, None),
        ];
        let rows = merge_snapshots(&pks, &snaps, &tree);
        let r1 = rows.iter().find(|r| r.span_pk == 1).unwrap();
        let r2 = rows.iter().find(|r| r.span_pk == 2).unwrap();
        assert!(!r1.is_sub_agent, "root-agent chat must not be sub-agent");
        assert!(r2.is_sub_agent, "nested-agent chat must be sub-agent");
    }

    #[test]
    fn cache_read_clamps_to_input() {
        // cache_read > input -> cacheR == input, fresh == 0.
        let row = MergedRow {
            span_pk: 1,
            token_limit: None,
            input_tokens: Some(100),
            output_tokens: Some(10),
            reasoning_tokens: Some(5),
            cache_read_tokens: Some(500),
            latest_ns: 0,
            is_sub_agent: false,
        };
        let (cache_r, fresh, out, rea) = split_segments(&row);
        assert_eq!(cache_r, 100);
        assert_eq!(fresh, 0);
        assert_eq!(out, 10);
        assert_eq!(rea, 5);
    }

    #[test]
    fn fresh_input_is_input_minus_cache() {
        let row = MergedRow {
            span_pk: 1,
            token_limit: None,
            input_tokens: Some(4000),
            output_tokens: None,
            reasoning_tokens: None,
            cache_read_tokens: Some(1000),
            latest_ns: 0,
            is_sub_agent: false,
        };
        let (cache_r, fresh, out, rea) = split_segments(&row);
        assert_eq!(cache_r, 1000);
        assert_eq!(fresh, 3000);
        assert_eq!(out, 0);
        assert_eq!(rea, 0);
    }

    #[test]
    fn max_current_tokens_picks_largest() {
        let snaps = vec![
            snap(Some(1), 1, None, Some(100), None, None, None, None),
            snap(Some(1), 2, None, Some(9000), None, None, None, None),
            snap(Some(1), 3, None, None, None, None, None, None),
        ];
        assert_eq!(max_current_tokens(&snaps), 9000);
    }
}
