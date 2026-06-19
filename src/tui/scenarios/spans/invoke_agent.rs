//! `Spans invoke_agent selection routes to latest chat descendant` — locate
//! the most recent chat descendant of an `invoke_agent` node by `sortKey`
//! (`end_unix_ns` → `start_unix_ns` → `span_pk`).

use crate::tui::model::{KindClass, SpanNode};

use super::follow_mode::sort_key;

/// Find the agent node by id, then walk its descendants for the latest
/// chat span. Returns `(trace_id, span_id)`.
pub fn latest_chat_descendant(tree: &[SpanNode], agent_span_id: &str) -> Option<(String, String)> {
    let agent = find_node(tree, agent_span_id)?;
    let mut best: Option<(&SpanNode, (i128, i128, i64))> = None;
    walk_descendants(agent, &mut best);
    best.map(|(n, _)| (n.trace_id.clone(), n.span_id.clone()))
}

fn walk_descendants<'a>(node: &'a SpanNode, best: &mut Option<(&'a SpanNode, (i128, i128, i64))>) {
    for c in &node.children {
        if matches!(c.kind_class, KindClass::Chat) {
            let k = sort_key(c);
            if best.map(|(_, b)| k > b).unwrap_or(true) {
                *best = Some((c, k));
            }
        }
        walk_descendants(c, best);
    }
}

fn find_node<'a>(tree: &'a [SpanNode], id: &str) -> Option<&'a SpanNode> {
    fn walk<'a>(n: &'a SpanNode, id: &str) -> Option<&'a SpanNode> {
        if n.span_id == id {
            return Some(n);
        }
        for c in &n.children {
            if let Some(x) = walk(c, id) {
                return Some(x);
            }
        }
        None
    }
    for r in tree {
        if let Some(n) = walk(r, id) {
            return Some(n);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::model::SpanProjection;

    fn mk(id: &str, kind: KindClass, end: i128, children: Vec<SpanNode>) -> SpanNode {
        SpanNode {
            span_pk: end as i64,
            trace_id: "t".into(),
            span_id: id.into(),
            parent_span_id: None,
            name: id.into(),
            kind_class: kind,
            ingestion_state: "complete".into(),
            error_type: None,
            status_code: None,
            start_unix_ns: None,
            end_unix_ns: Some(end),
            projection: SpanProjection::default(),
            children,
        }
    }

    #[test]
    fn picks_latest_chat_descendant_by_end() {
        let tree = vec![mk(
            "agent",
            KindClass::InvokeAgent,
            0,
            vec![
                mk(
                    "child",
                    KindClass::Other,
                    50,
                    vec![mk("chat_old", KindClass::Chat, 100, vec![])],
                ),
                mk("chat_new", KindClass::Chat, 200, vec![]),
            ],
        )];
        assert_eq!(
            latest_chat_descendant(&tree, "agent"),
            Some(("t".into(), "chat_new".into()))
        );
    }

    #[test]
    fn returns_none_when_no_chat_descendants() {
        let tree = vec![mk(
            "agent",
            KindClass::InvokeAgent,
            0,
            vec![mk("tool", KindClass::ExecuteTool, 100, vec![])],
        )];
        assert_eq!(latest_chat_descendant(&tree, "agent"), None);
    }

    #[test]
    fn returns_none_when_agent_id_missing() {
        let tree = vec![mk("other", KindClass::InvokeAgent, 0, vec![])];
        assert_eq!(latest_chat_descendant(&tree, "agent"), None);
    }
}
