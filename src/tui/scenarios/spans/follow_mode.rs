//! `Spans follows latest tool span` — pure helper that, given the current
//! `node_map` and `selected_span_id`, computes the latest tool span
//! (`execute_tool` or `external_tool`) by `sortKey` (`end_unix_ns` →
//! `start_unix_ns` → `span_pk`). Caller decides when to advance based on the
//! follow-mode toggle.

use crate::tui::model::{KindClass, SpanNode};

/// Sort key for span ordering. Mirrors the LLR's preference order.
pub fn sort_key(n: &SpanNode) -> (i128, i128, i64) {
    let end = n.end_unix_ns.unwrap_or(0);
    let start = n.start_unix_ns.unwrap_or(0);
    (end, start, n.span_pk)
}

/// Locate the latest tool span in the forest. Returns `Some((trace_id,
/// span_id))` when one exists.
pub fn latest_tool_span(tree: &[SpanNode]) -> Option<(String, String)> {
    let mut best: Option<(&SpanNode, (i128, i128, i64))> = None;
    fn walk<'a>(node: &'a SpanNode, best: &mut Option<(&'a SpanNode, (i128, i128, i64))>) {
        if matches!(
            node.kind_class,
            KindClass::ExecuteTool | KindClass::ExternalTool
        ) {
            let key = sort_key(node);
            if best.map(|(_, k)| key > k).unwrap_or(true) {
                *best = Some((node, key));
            }
        }
        for c in &node.children {
            walk(c, best);
        }
    }
    for r in tree {
        walk(r, &mut best);
    }
    best.map(|(n, _)| (n.trace_id.clone(), n.span_id.clone()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::model::SpanProjection;

    fn mk(id: &str, kind: KindClass, end: Option<i128>, span_pk: i64) -> SpanNode {
        SpanNode {
            span_pk,
            trace_id: "t".into(),
            span_id: id.into(),
            parent_span_id: None,
            name: id.into(),
            kind_class: kind,
            ingestion_state: "complete".into(),
            error_type: None,
            status_code: None,
            start_unix_ns: None,
            end_unix_ns: end,
            projection: SpanProjection::default(),
            children: vec![],
        }
    }

    #[test]
    fn picks_latest_by_end_ns() {
        let t = vec![
            mk("a", KindClass::ExecuteTool, Some(100), 1),
            mk("b", KindClass::ExecuteTool, Some(200), 2),
            mk("c", KindClass::Chat, Some(300), 3), // ignored — wrong kind
        ];
        assert_eq!(
            latest_tool_span(&t),
            Some(("t".to_string(), "b".to_string()))
        );
    }

    #[test]
    fn span_pk_tiebreaker() {
        let t = vec![
            mk("a", KindClass::ExecuteTool, Some(100), 1),
            mk("b", KindClass::ExecuteTool, Some(100), 2),
        ];
        assert_eq!(
            latest_tool_span(&t),
            Some(("t".to_string(), "b".to_string()))
        );
    }

    #[test]
    fn returns_none_for_no_tool_spans() {
        let t = vec![mk("c", KindClass::Chat, Some(100), 1)];
        assert_eq!(latest_tool_span(&t), None);
    }

    #[test]
    fn external_tool_counts() {
        let t = vec![mk("a", KindClass::ExternalTool, Some(100), 1)];
        assert!(latest_tool_span(&t).is_some());
    }
}
