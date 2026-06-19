//! `Spans execute_tool selection auto-advances chat detail` — pure function
//! implementing the 3-rule shape-aware dispatch (nested → sibling →
//! fallback). Vendor-agnostic; does NOT consult `service_name`.

use crate::tui::model::{KindClass, SpanNode};

use super::follow_mode::sort_key;

/// Locate the "following chat span" for a picked tool span. Returns
/// `Some((trace_id, span_id))` when one resolves; `None` otherwise. Caller
/// leaves the chat-detail selection unchanged on `None`.
pub fn find_following_chat_span(tree: &[SpanNode], picked_span_id: &str) -> Option<(String, String)> {
    let path = find_path(tree, picked_span_id)?;
    let picked = path.last().copied()?;

    // Rule 1: NESTED — picked has a chat-class ancestor in the path.
    let chat_ancestor = path
        .iter()
        .rev()
        .skip(1) // skip picked itself
        .find(|n| matches!(n.kind_class, KindClass::Chat));
    if let Some(ancestor) = chat_ancestor {
        let anc_key = sort_key(ancestor);
        let mut best: Option<(&SpanNode, (i128, i128, i64))> = None;
        for n in iter_all(tree) {
            if !matches!(n.kind_class, KindClass::Chat) {
                continue;
            }
            let k = sort_key(n);
            if k <= anc_key {
                continue;
            }
            if best.map(|(_, b)| k < b).unwrap_or(true) {
                best = Some((n, k));
            }
        }
        if let Some((n, _)) = best {
            return Some((n.trace_id.clone(), n.span_id.clone()));
        }
    }

    // Rule 2: SIBLING — first chat sibling temporally enclosing picked.
    let parent = if path.len() >= 2 { Some(path[path.len() - 2]) } else { None };
    let p_start = picked.start_unix_ns.unwrap_or(0);
    let p_end = picked.end_unix_ns.unwrap_or(0);
    if chat_ancestor.is_none() {
        if let Some(parent) = parent {
            for sib in &parent.children {
                if !matches!(sib.kind_class, KindClass::Chat) {
                    continue;
                }
                let s = sib.start_unix_ns.unwrap_or(0);
                let e = sib.end_unix_ns.unwrap_or(0);
                if s <= p_start && e >= p_end {
                    return Some((sib.trace_id.clone(), sib.span_id.clone()));
                }
            }
        } else {
            // Picked is a root → scan root siblings.
            for sib in tree {
                if sib.span_id == picked_span_id {
                    continue;
                }
                if !matches!(sib.kind_class, KindClass::Chat) {
                    continue;
                }
                let s = sib.start_unix_ns.unwrap_or(0);
                let e = sib.end_unix_ns.unwrap_or(0);
                if s <= p_start && e >= p_end {
                    return Some((sib.trace_id.clone(), sib.span_id.clone()));
                }
            }
        }
    }

    // Rule 3: FALLBACK — next chat sibling chronologically (sortKey >
    // picked).
    let picked_key = sort_key(picked);
    let siblings: &[SpanNode] = match parent {
        Some(p) => &p.children,
        None => tree,
    };
    let mut best: Option<(&SpanNode, (i128, i128, i64))> = None;
    for sib in siblings {
        if !matches!(sib.kind_class, KindClass::Chat) {
            continue;
        }
        let k = sort_key(sib);
        if k <= picked_key {
            continue;
        }
        if best.map(|(_, b)| k < b).unwrap_or(true) {
            best = Some((sib, k));
        }
    }
    best.map(|(n, _)| (n.trace_id.clone(), n.span_id.clone()))
}

/// Path of references from a root to the target node, inclusive of both.
fn find_path<'a>(tree: &'a [SpanNode], id: &str) -> Option<Vec<&'a SpanNode>> {
    fn walk<'a>(
        node: &'a SpanNode,
        id: &str,
        path: &mut Vec<&'a SpanNode>,
    ) -> bool {
        path.push(node);
        if node.span_id == id {
            return true;
        }
        for c in &node.children {
            if walk(c, id, path) {
                return true;
            }
        }
        path.pop();
        false
    }
    for r in tree {
        let mut path = Vec::new();
        if walk(r, id, &mut path) {
            return Some(path);
        }
    }
    None
}

fn iter_all(tree: &[SpanNode]) -> Vec<&SpanNode> {
    let mut out = Vec::new();
    fn walk<'a>(n: &'a SpanNode, out: &mut Vec<&'a SpanNode>) {
        out.push(n);
        for c in &n.children {
            walk(c, out);
        }
    }
    for r in tree {
        walk(r, &mut out);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::model::SpanProjection;

    fn mk(
        id: &str,
        kind: KindClass,
        start: i128,
        end: i128,
        children: Vec<SpanNode>,
    ) -> SpanNode {
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
            start_unix_ns: Some(start),
            end_unix_ns: Some(end),
            projection: SpanProjection::default(),
            children,
        }
    }

    #[test]
    fn nested_picks_smallest_chat_sortkey_after_ancestor() {
        // chat_anc (end=100) → exec_tool (end=110)
        // siblings: chat_a (end=150), chat_b (end=200)
        // → expected chat_a (150 > 100, smallest > 100)
        let tree = vec![
            mk(
                "chat_anc",
                KindClass::Chat,
                10,
                100,
                vec![mk("tool", KindClass::ExecuteTool, 50, 110, vec![])],
            ),
            mk("chat_a", KindClass::Chat, 110, 150, vec![]),
            mk("chat_b", KindClass::Chat, 160, 200, vec![]),
        ];
        let r = find_following_chat_span(&tree, "tool");
        assert_eq!(r, Some(("t".into(), "chat_a".into())));
    }

    #[test]
    fn sibling_shape_chat_encloses_picked() {
        // parent has chat_sib (start=0, end=200) and tool (start=50, end=100).
        // tool has no chat ancestor → SIBLING shape.
        let tree = vec![mk(
            "parent",
            KindClass::Other,
            0,
            300,
            vec![
                mk("chat_sib", KindClass::Chat, 0, 200, vec![]),
                mk("tool", KindClass::ExecuteTool, 50, 100, vec![]),
            ],
        )];
        let r = find_following_chat_span(&tree, "tool");
        assert_eq!(r, Some(("t".into(), "chat_sib".into())));
    }

    #[test]
    fn fallback_to_next_chronological_chat_sibling() {
        // parent has tool (end=100), chat_late (end=200). No enclosing
        // chat sibling and no chat ancestor → FALLBACK.
        let tree = vec![mk(
            "parent",
            KindClass::Other,
            0,
            300,
            vec![
                mk("tool", KindClass::ExecuteTool, 50, 100, vec![]),
                mk("chat_late", KindClass::Chat, 110, 200, vec![]),
            ],
        )];
        let r = find_following_chat_span(&tree, "tool");
        assert_eq!(r, Some(("t".into(), "chat_late".into())));
    }

    #[test]
    fn no_match_returns_none() {
        let tree = vec![mk(
            "parent",
            KindClass::Other,
            0,
            300,
            vec![mk("tool", KindClass::ExecuteTool, 50, 100, vec![])],
        )];
        assert_eq!(find_following_chat_span(&tree, "tool"), None);
    }
}
