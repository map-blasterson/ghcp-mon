//! Pure tree construction for Chat Detail.
//!
//! Builds the 4-branch breakdown tree (system instructions / tool definitions /
//! input messages / output messages) for either FULL or DELTA mode. Each
//! node carries a `NodeId` (slash-delimited path), a label, optional meta, the
//! byte size computed as `JSON.stringify(value).len()`, an optional diff
//! badge, a typed [`NodeKind`] for renderer dispatch, children, and a list of
//! "primitive" (key, raw-value) entries that the renderer surfaces as
//! key=value rows (with click-to-expand for long primitives).
//!
//! Source for (shared `frontend/llr/`):
//! - `Chat detail tree built from four content attributes`
//! - `Chat detail bytes computed via JSON length`
//! - `Chat detail DELTA diffs against prior chat span`
//! - `Chat detail system instructions word-diff in DELTA`
//! - `Chat detail tool defs name-diff in DELTA`
//! - `Chat detail DELTA input messages carried-forward suffix`

use serde_json::Value;
use std::collections::HashMap;

use crate::tui::scenarios::chat_detail::diff_segments::{
    all_unchanged, count_added, count_removed, word_diff, DiffSegment,
};
use crate::tui::scenarios::chat_detail::messages::{
    parse_input_messages, parse_output_messages, Message, Part,
};

/// Slash-delimited stable node path. Preserved across rebuilds so per-node
/// state (expand toggles, expanded primitives, per-block search state) stays
/// attached to the same node when the tree is regenerated.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct NodeId(pub String);

impl NodeId {
    pub fn root() -> Self {
        NodeId("root".to_string())
    }
    pub fn child(&self, seg: &str) -> Self {
        NodeId(format!("{}/{}", self.0, seg))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for NodeId {
    fn from(s: &str) -> Self {
        NodeId(s.to_string())
    }
}

/// Render-time mode chip.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChatMode {
    Delta,
    Full,
}

impl ChatMode {
    pub fn from_config_str(s: Option<&str>) -> Self {
        match s {
            Some("FULL") => ChatMode::Full,
            _ => ChatMode::Delta,
        }
    }
    pub fn label(&self) -> &'static str {
        match self {
            ChatMode::Delta => "DELTA",
            ChatMode::Full => "FULL",
        }
    }
    pub fn toggled(self) -> Self {
        match self {
            ChatMode::Delta => ChatMode::Full,
            ChatMode::Full => ChatMode::Delta,
        }
    }
}

/// Optional badge displayed next to a node label.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeBadge {
    Unchanged,
    Changed,
    Added,
    Removed,
}

/// Typed discriminator carrying any per-branch payload needed by the
/// renderer (no bg-color sentinels — every visual branch maps to a variant).
#[derive(Debug, Clone, PartialEq)]
pub enum NodeKind {
    /// The 4-branch root.
    Root,
    /// One of the four top-level sections (`system instructions` etc.) in
    /// FULL mode or a non-diff DELTA case.
    Section,
    /// FULL mode: the system instructions parts container.
    SystemParts,
    /// DELTA: equal-system-instructions sentinel.
    SystemUnchanged,
    /// DELTA: changed-system-instructions wrapper (carries a `SystemDiff`
    /// child).
    SystemChanged,
    /// DELTA: leaf carrying the actual word-diff segments.
    SystemDiff(Vec<DiffSegment>),
    /// FULL: each tool def has its own node.
    ToolDef { name: String },
    /// DELTA: all tool defs were unchanged.
    ToolDefUnchanged,
    /// DELTA: tool def present in prior but missing in current.
    ToolDefRemoved { name: String },
    /// DELTA: tool def new in current.
    ToolDefAdded { name: String },
    /// DELTA: input-messages carried-forward sentinel.
    InputMessagesUnchanged,
    /// One chat message.
    Message { role: String, finish_reason: Option<String> },
    /// One part of a message (`text` / `reasoning` etc.).
    Part(Part),
    /// Generic leaf used for primitive containers.
    Leaf,
}

/// A tree node. Children render in order; `primitives` render after a node's
/// own label, before its children, as `key = value` rows.
#[derive(Debug, Clone)]
pub struct TreeNode {
    pub id: NodeId,
    pub label: String,
    pub meta: Option<String>,
    pub bytes: usize,
    pub badge: Option<NodeBadge>,
    pub kind: NodeKind,
    pub children: Vec<TreeNode>,
    /// `(key, raw value)` pairs surfaced as the node's primitives. The
    /// renderer treats the value as collapsible when its stringified length
    /// exceeds 200 chars or contains a `\n`.
    pub primitives: Vec<(String, Value)>,
}

impl TreeNode {
    fn new(id: NodeId, label: impl Into<String>, kind: NodeKind) -> Self {
        Self {
            id,
            label: label.into(),
            meta: None,
            bytes: 0,
            badge: None,
            kind,
            children: Vec::new(),
            primitives: Vec::new(),
        }
    }
    fn with_meta(mut self, meta: impl Into<String>) -> Self {
        self.meta = Some(meta.into());
        self
    }
    fn with_bytes(mut self, n: usize) -> Self {
        self.bytes = n;
        self
    }
    fn with_badge(mut self, b: NodeBadge) -> Self {
        self.badge = Some(b);
        self
    }
}

/// Captured chat content used as the DELTA baseline.
#[derive(Debug, Clone, Default)]
pub struct ChatContent {
    pub system: Vec<Value>,
    pub tool_defs: Vec<Value>,
    pub input_messages: Vec<Message>,
    pub output_messages: Vec<Message>,
}

impl ChatContent {
    /// Extract the four content arrays from a span's attribute map.
    pub fn from_attrs(attrs: &Value) -> Self {
        Self {
            system: parse_value_array(attrs, "gen_ai.system_instructions"),
            tool_defs: parse_value_array(attrs, "gen_ai.tool.definitions"),
            input_messages: parse_input_messages(attrs),
            output_messages: parse_output_messages(attrs),
        }
    }

    /// True iff at least one of system/input/output is non-empty (used by
    /// DELTA to decide "prior captured content is empty → degrade to FULL").
    pub fn is_empty(&self) -> bool {
        self.system.is_empty()
            && self.input_messages.is_empty()
            && self.output_messages.is_empty()
    }
}

/// Accept inline array or JSON-stringified array; return empty on absent /
/// malformed.
fn parse_value_array(attrs: &Value, key: &str) -> Vec<Value> {
    let Some(v) = attrs.as_object().and_then(|o| o.get(key)) else {
        return Vec::new();
    };
    match v {
        Value::Array(a) => a.clone(),
        Value::String(s) => serde_json::from_str::<Value>(s)
            .ok()
            .and_then(|p| p.as_array().cloned())
            .unwrap_or_default(),
        _ => Vec::new(),
    }
}

/// `JSON.stringify(value).len()` with safe fallback to 0. Mirrors the LLR's
/// "fall back to 0 on serialize error" requirement.
pub fn json_bytes(v: &Value) -> usize {
    serde_json::to_string(v).map(|s| s.len()).unwrap_or(0)
}

/// Bytes for an array, costed as the JSON encoding of the whole array.
pub fn array_bytes(arr: &[Value]) -> usize {
    json_bytes(&Value::Array(arr.to_vec()))
}

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// Build the 4-branch root tree. When `prior` is `Some` AND `mode == Delta`,
/// system instructions and tool definitions get DELTA-mode treatment and
/// input messages get carried-forward-suffix detection.
pub fn build_tree(current: &ChatContent, prior: Option<&ChatContent>, mode: ChatMode) -> TreeNode {
    let root_id = NodeId::root();
    let prior_for_delta = if matches!(mode, ChatMode::Delta) { prior } else { None };

    let sys = build_system_node(&root_id.child("system"), &current.system, prior_for_delta);
    let tools = build_tool_defs_node(
        &root_id.child("tools"),
        &current.tool_defs,
        prior_for_delta,
    );
    let input = build_input_messages_node(
        &root_id.child("input"),
        &current.input_messages,
        prior_for_delta,
    );
    let output = build_output_messages_node(
        &root_id.child("output"),
        &current.output_messages,
    );

    let total = sys.bytes + tools.bytes + input.bytes + output.bytes;
    let mut root = TreeNode::new(root_id, "chat", NodeKind::Root).with_bytes(total);
    root.children = vec![sys, tools, input, output];
    root
}

// ---------------------------------------------------------------------------
// system instructions
// ---------------------------------------------------------------------------

/// Concatenate the text/reasoning content of system-instruction parts. The
/// LLR specifies the diff input is the concatenated body of each side.
fn system_body(parts: &[Value]) -> String {
    let mut out = String::new();
    for p in parts {
        let ty = p.get("type").and_then(Value::as_str).unwrap_or("");
        if matches!(ty, "text" | "reasoning") {
            if let Some(c) = p.get("content").and_then(Value::as_str) {
                if !out.is_empty() {
                    out.push('\n');
                }
                out.push_str(c);
            }
        }
    }
    out
}

pub fn build_system_node(
    id: &NodeId,
    current: &[Value],
    prior: Option<&ChatContent>,
) -> TreeNode {
    let bytes = array_bytes(current);
    let count = current.len();
    let label = "system instructions".to_string();

    if let Some(p) = prior {
        // DELTA path.
        if p.system == current.to_vec() {
            return TreeNode::new(
                id.child("unchanged"),
                label.clone(),
                NodeKind::SystemUnchanged,
            )
            .with_meta(format!("unchanged · {count} part{}", plural_s(count)))
            .with_bytes(bytes)
            .with_badge(NodeBadge::Unchanged);
        }
        // Word-diff over concatenated body.
        let prior_body = system_body(&p.system);
        let cur_body = system_body(current);
        let segs = word_diff(&prior_body, &cur_body);
        if all_unchanged(&segs) {
            // Bodies identical but parts arrays differ in metadata — still
            // surface as unchanged (no visible word-level change).
            return TreeNode::new(
                id.child("unchanged"),
                label,
                NodeKind::SystemUnchanged,
            )
            .with_meta(format!("unchanged · {count} part{}", plural_s(count)))
            .with_bytes(bytes)
            .with_badge(NodeBadge::Unchanged);
        }
        let added = count_added(&segs);
        let removed = count_removed(&segs);
        let mut diff_node = TreeNode::new(
            id.child("diff"),
            "system diff",
            NodeKind::SystemDiff(segs),
        )
        .with_meta(format!("+{added} ch · -{removed} ch"))
        .with_bytes(bytes);
        diff_node.badge = Some(NodeBadge::Changed);
        let mut node = TreeNode::new(id.clone(), label, NodeKind::SystemChanged)
            .with_meta(format!("{count} part{} · changed", plural_s(count)))
            .with_bytes(bytes)
            .with_badge(NodeBadge::Changed);
        node.children.push(diff_node);
        return node;
    }

    // FULL path: one child per part, primitives for type/content key=value.
    let mut node = TreeNode::new(id.clone(), label, NodeKind::SystemParts)
        .with_meta(format!("{count} part{}", plural_s(count)))
        .with_bytes(bytes);
    for (i, p) in current.iter().enumerate() {
        let child_id = id.child(&i.to_string());
        let pbytes = json_bytes(p);
        let part_label = p
            .get("type")
            .and_then(Value::as_str)
            .unwrap_or("part")
            .to_string();
        let mut child = TreeNode::new(child_id, part_label, NodeKind::Leaf).with_bytes(pbytes);
        if let Some(obj) = p.as_object() {
            for (k, v) in obj {
                child.primitives.push((k.clone(), v.clone()));
            }
        }
        node.children.push(child);
    }
    node
}

// ---------------------------------------------------------------------------
// tool definitions
// ---------------------------------------------------------------------------

fn tool_def_name(v: &Value) -> Option<String> {
    v.get("name").and_then(Value::as_str).map(|s| s.to_string())
}

pub fn build_tool_defs_node(
    id: &NodeId,
    current: &[Value],
    prior: Option<&ChatContent>,
) -> TreeNode {
    let bytes = array_bytes(current);
    let label = "tool definitions".to_string();
    let count = current.len();

    if let Some(p) = prior {
        // Group by name (Option<String>). Unnamed entries match by
        // deep-equality multiset.
        let mut cur_named: HashMap<String, Vec<Value>> = HashMap::new();
        let mut cur_unnamed: Vec<Value> = Vec::new();
        for d in current {
            match tool_def_name(d) {
                Some(n) => cur_named.entry(n).or_default().push(d.clone()),
                None => cur_unnamed.push(d.clone()),
            }
        }
        let mut prior_named: HashMap<String, Vec<Value>> = HashMap::new();
        let mut prior_unnamed: Vec<Value> = Vec::new();
        for d in &p.tool_defs {
            match tool_def_name(d) {
                Some(n) => prior_named.entry(n).or_default().push(d.clone()),
                None => prior_unnamed.push(d.clone()),
            }
        }

        let mut added: Vec<(String, Value)> = Vec::new();
        let mut removed: Vec<(String, Value)> = Vec::new();
        let mut all_names: Vec<String> = cur_named.keys().chain(prior_named.keys()).cloned().collect();
        all_names.sort();
        all_names.dedup();
        for name in all_names {
            let cur = cur_named.remove(&name).unwrap_or_default();
            let pr = prior_named.remove(&name).unwrap_or_default();
            // Per-name multiset diff: matching entries cancel out.
            let mut cur_remaining = cur.clone();
            for entry in &pr {
                if let Some(pos) = cur_remaining.iter().position(|c| c == entry) {
                    cur_remaining.remove(pos);
                } else {
                    removed.push((name.clone(), entry.clone()));
                }
            }
            for entry in cur_remaining {
                // Either net-new under this name OR same-name-different-content.
                added.push((name.clone(), entry));
            }
            // If there are "extra" priors not yet matched (same-name but
            // current already exhausted matches) they got removed above.
            // But our greedy multiset removed them already; nothing more.
            // Symmetric: if cur has more than prior with the same content, the
            // remaining cur entries are surplus → added. Handled above.
        }
        // Unnamed entries: multiset by deep equality.
        let mut cur_un = cur_unnamed.clone();
        for entry in &prior_unnamed {
            if let Some(pos) = cur_un.iter().position(|c| c == entry) {
                cur_un.remove(pos);
            } else {
                removed.push(("(unnamed)".to_string(), entry.clone()));
            }
        }
        for entry in cur_un {
            added.push(("(unnamed)".to_string(), entry));
        }

        if added.is_empty() && removed.is_empty() {
            return TreeNode::new(
                id.child("unchanged"),
                label,
                NodeKind::ToolDefUnchanged,
            )
            .with_meta(format!("unchanged · {count} def{}", plural_s(count)))
            .with_bytes(bytes)
            .with_badge(NodeBadge::Unchanged);
        }

        let mut root = TreeNode::new(id.clone(), label, NodeKind::Section)
            .with_meta(format!(
                "{} removed · {} added",
                removed.len(),
                added.len()
            ))
            .with_bytes(bytes)
            .with_badge(NodeBadge::Changed);
        // REMOVED first, then ADDED, per the LLR.
        for (i, (name, v)) in removed.into_iter().enumerate() {
            let cid = id.child(&format!("rem{i}"));
            let mut node = TreeNode::new(
                cid,
                name.clone(),
                NodeKind::ToolDefRemoved { name },
            )
            .with_bytes(json_bytes(&v))
            .with_badge(NodeBadge::Removed);
            if let Some(obj) = v.as_object() {
                for (k, vv) in obj {
                    node.primitives.push((k.clone(), vv.clone()));
                }
            }
            root.children.push(node);
        }
        for (i, (name, v)) in added.into_iter().enumerate() {
            let cid = id.child(&format!("add{i}"));
            let mut node = TreeNode::new(
                cid,
                name.clone(),
                NodeKind::ToolDefAdded { name },
            )
            .with_bytes(json_bytes(&v))
            .with_badge(NodeBadge::Added);
            if let Some(obj) = v.as_object() {
                for (k, vv) in obj {
                    node.primitives.push((k.clone(), vv.clone()));
                }
            }
            root.children.push(node);
        }
        return root;
    }

    // FULL path.
    let mut node = TreeNode::new(id.clone(), label, NodeKind::Section)
        .with_meta(format!("{count} def{}", plural_s(count)))
        .with_bytes(bytes);
    for (i, d) in current.iter().enumerate() {
        let cid = id.child(&i.to_string());
        let name = tool_def_name(d).unwrap_or_else(|| format!("def {i}"));
        let mut child = TreeNode::new(cid, name.clone(), NodeKind::ToolDef { name })
            .with_bytes(json_bytes(d));
        if let Some(obj) = d.as_object() {
            for (k, v) in obj {
                child.primitives.push((k.clone(), v.clone()));
            }
        }
        node.children.push(child);
    }
    node
}

// ---------------------------------------------------------------------------
// input messages (with carried-forward suffix detection)
// ---------------------------------------------------------------------------

fn message_to_value(m: &Message) -> Value {
    m.raw.clone()
}

fn common_prefix_len(current: &[Message], prior: &[Message]) -> usize {
    let n = current.len().min(prior.len());
    for i in 0..n {
        if message_to_value(&current[i]) != message_to_value(&prior[i]) {
            return i;
        }
    }
    n
}

pub fn build_input_messages_node(
    id: &NodeId,
    current: &[Message],
    prior: Option<&ChatContent>,
) -> TreeNode {
    let raws: Vec<Value> = current.iter().map(message_to_value).collect();
    let bytes = array_bytes(&raws);
    let count = current.len();
    let label = "input messages".to_string();

    if let Some(p) = prior {
        if !p.input_messages.is_empty() {
            let prefix_len = common_prefix_len(current, &p.input_messages);
            if prefix_len == p.input_messages.len() && current.len() > prefix_len {
                // Carried-forward suffix detected.
                let mut node = TreeNode::new(id.clone(), label, NodeKind::Section)
                    .with_meta(format!(
                        "carried forward · {} new",
                        current.len() - prefix_len
                    ))
                    .with_bytes(bytes)
                    .with_badge(NodeBadge::Changed);
                node.children.push(
                    TreeNode::new(
                        id.child("unchanged"),
                        "carried forward".to_string(),
                        NodeKind::InputMessagesUnchanged,
                    )
                    .with_meta(format!(
                        "unchanged · {prefix_len} message(s) from prior turn"
                    ))
                    .with_bytes(array_bytes(
                        &p.input_messages
                            .iter()
                            .map(message_to_value)
                            .collect::<Vec<_>>(),
                    ))
                    .with_badge(NodeBadge::Unchanged),
                );
                for (j, m) in current.iter().skip(prefix_len).enumerate() {
                    let orig_idx = prefix_len + j;
                    node.children.push(build_message_node(
                        &id.child(&format!("input_messages/{orig_idx}")),
                        m,
                        Some(NodeBadge::Added),
                    ));
                }
                return node;
            }
        }
        // Non-strict-prefix DELTA path: full per-turn delta.
        let mut node = TreeNode::new(id.clone(), label, NodeKind::Section)
            .with_meta(format!("{count} message(s) · per-turn delta", count = count))
            .with_bytes(bytes);
        for (i, m) in current.iter().enumerate() {
            node.children.push(build_message_node(
                &id.child(&format!("input_messages/{i}")),
                m,
                None,
            ));
        }
        return node;
    }

    // FULL path.
    let mut node = TreeNode::new(id.clone(), label, NodeKind::Section)
        .with_meta(format!("{count} message{}", plural_s(count)))
        .with_bytes(bytes);
    for (i, m) in current.iter().enumerate() {
        node.children.push(build_message_node(
            &id.child(&format!("input_messages/{i}")),
            m,
            None,
        ));
    }
    node
}

pub fn build_output_messages_node(id: &NodeId, current: &[Message]) -> TreeNode {
    let raws: Vec<Value> = current.iter().map(message_to_value).collect();
    let bytes = array_bytes(&raws);
    let count = current.len();
    let mut node = TreeNode::new(id.clone(), "output messages", NodeKind::Section)
        .with_meta(format!("{count} message{}", plural_s(count)))
        .with_bytes(bytes);
    for (i, m) in current.iter().enumerate() {
        node.children.push(build_message_node(
            &id.child(&format!("output_messages/{i}")),
            m,
            None,
        ));
    }
    node
}

fn build_message_node(id: &NodeId, m: &Message, badge: Option<NodeBadge>) -> TreeNode {
    let bytes = json_bytes(&m.raw);
    let label = format!("message · {}", m.role);
    let meta = m.finish_reason.clone();
    let mut node = TreeNode::new(
        id.clone(),
        label,
        NodeKind::Message {
            role: m.role.clone(),
            finish_reason: m.finish_reason.clone(),
        },
    )
    .with_bytes(bytes);
    if let Some(m) = meta {
        node = node.with_meta(format!("finish_reason: {m}"));
    }
    if let Some(b) = badge {
        node = node.with_badge(b);
    }
    for (i, p) in m.parts.iter().enumerate() {
        let cid = id.child(&format!("parts/{i}"));
        let pbytes = json_bytes(p.raw());
        let plabel = match p {
            Part::Text { .. } => "text".to_string(),
            Part::Reasoning { .. } => "reasoning".to_string(),
            Part::ToolCall { name, .. } => format!("tool_call · {name}"),
            Part::ToolCallResponse { id, .. } => format!("tool_call_response · {}", short_id(id)),
            Part::Other { raw } => raw
                .get("type")
                .and_then(Value::as_str)
                .unwrap_or("part")
                .to_string(),
        };
        let mut pnode = TreeNode::new(cid, plabel, NodeKind::Part(p.clone())).with_bytes(pbytes);
        if let Some(obj) = p.raw().as_object() {
            for (k, v) in obj {
                pnode.primitives.push((k.clone(), v.clone()));
            }
        }
        node.children.push(pnode);
    }
    node
}

fn short_id(s: &str) -> String {
    let n = 8.min(s.len());
    s[..n].to_string()
}

fn plural_s(n: usize) -> &'static str {
    if n == 1 { "" } else { "s" }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn full_empty_yields_four_branches_root_sum() {
        let c = ChatContent::default();
        let t = build_tree(&c, None, ChatMode::Full);
        assert_eq!(t.children.len(), 4);
        // Each child renders an empty JSON array (`"[]"`) = 2 bytes per the
        // `JSON.stringify(value).length` rule.
        let sum: usize = t.children.iter().map(|c| c.bytes).sum();
        assert_eq!(t.bytes, sum);
        assert_eq!(t.children[0].label, "system instructions");
        assert_eq!(t.children[1].label, "tool definitions");
        assert_eq!(t.children[2].label, "input messages");
        assert_eq!(t.children[3].label, "output messages");
    }

    #[test]
    fn full_non_empty_root_sum() {
        let attrs = json!({
            "gen_ai.system_instructions": [{"type":"text","content":"hi"}],
            "gen_ai.tool.definitions": [{"name":"ls"}],
            "gen_ai.input.messages": [{"role":"user","parts":[{"type":"text","content":"x"}]}],
            "gen_ai.output.messages": []
        });
        let c = ChatContent::from_attrs(&attrs);
        let t = build_tree(&c, None, ChatMode::Full);
        let sum: usize = t.children.iter().map(|c| c.bytes).sum();
        assert_eq!(t.bytes, sum);
        assert!(t.bytes > 0);
    }

    #[test]
    fn delta_without_prior_degrades_to_full() {
        let attrs = json!({
            "gen_ai.system_instructions": [{"type":"text","content":"hi"}],
        });
        let c = ChatContent::from_attrs(&attrs);
        let t = build_tree(&c, None, ChatMode::Delta);
        // No diff badges anywhere.
        fn walk(n: &TreeNode) -> bool {
            n.badge.is_none() && n.children.iter().all(walk)
        }
        assert!(walk(&t));
    }

    #[test]
    fn delta_system_unchanged_emits_sentinel() {
        let parts = vec![json!({"type":"text","content":"hello"})];
        let cur = ChatContent { system: parts.clone(), ..Default::default() };
        let prior = ChatContent { system: parts, ..Default::default() };
        let n = build_system_node(&NodeId::root().child("system"), &cur.system, Some(&prior));
        assert!(matches!(n.kind, NodeKind::SystemUnchanged));
        assert_eq!(n.badge, Some(NodeBadge::Unchanged));
    }

    #[test]
    fn delta_system_changed_carries_diff_child() {
        let cur = ChatContent {
            system: vec![json!({"type":"text","content":"hello world"})],
            ..Default::default()
        };
        let prior = ChatContent {
            system: vec![json!({"type":"text","content":"hello there"})],
            ..Default::default()
        };
        let n = build_system_node(&NodeId::root().child("system"), &cur.system, Some(&prior));
        assert!(matches!(n.kind, NodeKind::SystemChanged));
        assert_eq!(n.badge, Some(NodeBadge::Changed));
        assert_eq!(n.children.len(), 1);
        assert!(matches!(&n.children[0].kind, NodeKind::SystemDiff(_)));
        let meta = n.children[0].meta.as_deref().unwrap_or("");
        assert!(meta.contains("ch ·") || meta.contains("+0") || meta.contains("-0"));
    }

    #[test]
    fn delta_tool_defs_all_unchanged() {
        let defs = vec![json!({"name":"ls","description":"d"})];
        let cur = ChatContent { tool_defs: defs.clone(), ..Default::default() };
        let prior = ChatContent { tool_defs: defs, ..Default::default() };
        let n = build_tool_defs_node(&NodeId::root().child("tools"), &cur.tool_defs, Some(&prior));
        assert!(matches!(n.kind, NodeKind::ToolDefUnchanged));
    }

    #[test]
    fn delta_tool_defs_same_name_diff_content_emits_removed_and_added() {
        let cur = ChatContent {
            tool_defs: vec![json!({"name":"ls","description":"new"})],
            ..Default::default()
        };
        let prior = ChatContent {
            tool_defs: vec![json!({"name":"ls","description":"old"})],
            ..Default::default()
        };
        let n = build_tool_defs_node(&NodeId::root().child("tools"), &cur.tool_defs, Some(&prior));
        let kinds: Vec<_> = n.children.iter().map(|c| c.badge).collect();
        assert!(kinds.contains(&Some(NodeBadge::Removed)));
        assert!(kinds.contains(&Some(NodeBadge::Added)));
        // REMOVED comes first per LLR.
        assert_eq!(n.children[0].badge, Some(NodeBadge::Removed));
    }

    #[test]
    fn delta_tool_defs_missing_in_current_is_removed() {
        let cur = ChatContent { tool_defs: vec![], ..Default::default() };
        let prior = ChatContent {
            tool_defs: vec![json!({"name":"ls"})],
            ..Default::default()
        };
        let n = build_tool_defs_node(&NodeId::root().child("tools"), &cur.tool_defs, Some(&prior));
        assert!(n.children.iter().any(|c| c.badge == Some(NodeBadge::Removed)));
    }

    #[test]
    fn delta_tool_defs_new_in_current_is_added() {
        let cur = ChatContent {
            tool_defs: vec![json!({"name":"new"})],
            ..Default::default()
        };
        let prior = ChatContent { tool_defs: vec![], ..Default::default() };
        let n = build_tool_defs_node(&NodeId::root().child("tools"), &cur.tool_defs, Some(&prior));
        assert!(n.children.iter().any(|c| c.badge == Some(NodeBadge::Added)));
    }

    #[test]
    fn delta_input_carried_forward_suffix() {
        let prior_msgs = vec![
            json!({"role":"user","parts":[{"type":"text","content":"q1"}]}),
            json!({"role":"assistant","parts":[{"type":"text","content":"a1"}]}),
        ];
        let cur_msgs: Vec<Value> = prior_msgs
            .iter()
            .cloned()
            .chain(std::iter::once(json!({
                "role":"user","parts":[{"type":"text","content":"q2"}]
            })))
            .collect();
        let prior_attrs = json!({"gen_ai.input.messages": prior_msgs});
        let cur_attrs = json!({"gen_ai.input.messages": cur_msgs});
        let cur = ChatContent::from_attrs(&cur_attrs);
        let prior = ChatContent::from_attrs(&prior_attrs);
        let n = build_input_messages_node(
            &NodeId::root().child("input"),
            &cur.input_messages,
            Some(&prior),
        );
        // First child = carried-forward sentinel; suffix indices are preserved.
        assert!(matches!(n.children[0].kind, NodeKind::InputMessagesUnchanged));
        assert!(n.children[1].id.as_str().ends_with("/input_messages/2"));
        assert_eq!(n.children[1].badge, Some(NodeBadge::Added));
    }

    #[test]
    fn delta_input_non_strict_prefix_is_per_turn_delta() {
        let prior = ChatContent::from_attrs(&json!({
            "gen_ai.input.messages": [
                {"role":"user","parts":[{"type":"text","content":"hi"}]}
            ]
        }));
        let cur = ChatContent::from_attrs(&json!({
            "gen_ai.input.messages": [
                {"role":"user","parts":[{"type":"text","content":"different"}]}
            ]
        }));
        let n = build_input_messages_node(
            &NodeId::root().child("input"),
            &cur.input_messages,
            Some(&prior),
        );
        // No carried-forward sentinel; meta says per-turn delta.
        assert!(!n.children.iter().any(|c| matches!(c.kind, NodeKind::InputMessagesUnchanged)));
        assert!(n.meta.as_deref().unwrap_or("").contains("per-turn delta"));
    }

    #[test]
    fn delta_input_empty_prior_skips_suffix_optimization() {
        let prior = ChatContent::default();
        let cur = ChatContent::from_attrs(&json!({
            "gen_ai.input.messages": [
                {"role":"user","parts":[{"type":"text","content":"hi"}]}
            ]
        }));
        let n = build_input_messages_node(
            &NodeId::root().child("input"),
            &cur.input_messages,
            Some(&prior),
        );
        assert!(!n.children.iter().any(|c| matches!(c.kind, NodeKind::InputMessagesUnchanged)));
    }

    #[test]
    fn json_bytes_matches_serde_to_string_length() {
        let v = json!({"a": 1, "b": [2, 3]});
        let exp = serde_json::to_string(&v).unwrap().len();
        assert_eq!(json_bytes(&v), exp);
    }

    #[test]
    fn root_node_id_is_root() {
        let c = ChatContent::default();
        let t = build_tree(&c, None, ChatMode::Full);
        assert_eq!(t.id.as_str(), "root");
        assert_eq!(t.children[0].id.as_str(), "root/system");
        assert_eq!(t.children[2].id.as_str(), "root/input");
    }
}
