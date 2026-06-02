//! Pure counting-tree builder for the File Touches column.
//!
//! Takes a flat list of [`Touch`] records (one per extracted file path) and
//! assembles a filesystem tree where every ancestor directory aggregates the
//! `reads`/`writes` counters of its descendants and each leaf collects the
//! [`TouchRef`]s that produced it. Children at every level are sorted
//! directories-first, then case-insensitive alphabetical by name.
//!
//! Source for (shared `frontend/llr/`):
//! - `File touches builds filesystem tree with counts`
//! - `File touches sort directories first then alphabetical`

/// Read vs write classification of a single touch. `read`-kind tool calls map
/// to [`TouchKind::Read`]; `write`/`edit`/`patch`-kind to [`TouchKind::Write`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TouchKind {
    Read,
    Write,
}

/// Back-reference to the span that produced a touch, retained on the leaf node
/// for future selection-routing. Today the renderer only counts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TouchRef {
    pub span_id: String,
    pub trace_id: String,
    pub span_pk: i64,
}

/// A single file-path touch extracted from a tool span.
#[derive(Debug, Clone)]
pub struct Touch {
    pub path: String,
    pub kind: TouchKind,
    pub span_ref: TouchRef,
}

/// Whether a tree node represents a directory or a file. A node created as an
/// intermediate path segment is always a [`NodeKind::Dir`]; a leaf segment is a
/// [`NodeKind::File`] unless it later gains children.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeKind {
    Dir,
    File,
}

/// One node in the file-touch tree.
#[derive(Debug, Clone)]
pub struct TouchNode {
    /// Final path segment (the display name).
    pub name: String,
    /// Full slash-delimited path from the tree root (the stable identity used
    /// for the open-directory set).
    pub path: String,
    pub kind: NodeKind,
    pub reads: u64,
    pub writes: u64,
    /// Span back-references; populated only on leaf (file) nodes.
    pub file_touches: Vec<TouchRef>,
    pub children: Vec<TouchNode>,
}

impl TouchNode {
    /// True when this node should be treated as a directory for sorting and
    /// expansion: either explicitly a [`NodeKind::Dir`] or it has children.
    pub fn is_dir(&self) -> bool {
        self.kind == NodeKind::Dir || !self.children.is_empty()
    }
}

/// The assembled file-touch tree (a forest of top-level nodes).
#[derive(Debug, Clone, Default)]
pub struct TouchTree {
    pub root: Vec<TouchNode>,
}

/// Build the counting tree from a flat list of touches.
///
/// For each touch: the path is split on `/` with empty segments dropped
/// (collapsing repeated separators and trimming leading/trailing `/`); an empty
/// path is skipped. Each segment node along the path has its `reads`/`writes`
/// counter incremented, and the leaf node collects the touch's [`TouchRef`].
/// Children at every level are then sorted directories-first then
/// case-insensitive alphabetical by name.
pub fn build_tree(touches: &[Touch]) -> TouchTree {
    let mut root: Vec<TouchNode> = Vec::new();
    for t in touches {
        let segs: Vec<&str> = t.path.split('/').filter(|s| !s.is_empty()).collect();
        if segs.is_empty() {
            continue;
        }
        insert(&mut root, &segs, 0, "", t);
    }
    sort_nodes(&mut root);
    TouchTree { root }
}

fn insert(level: &mut Vec<TouchNode>, segs: &[&str], i: usize, parent_path: &str, t: &Touch) {
    let name = segs[i];
    let path = if parent_path.is_empty() {
        name.to_string()
    } else {
        format!("{parent_path}/{name}")
    };
    let is_last = i + 1 == segs.len();

    let idx = match level.iter().position(|n| n.name == name) {
        Some(idx) => idx,
        None => {
            level.push(TouchNode {
                name: name.to_string(),
                path: path.clone(),
                kind: if is_last { NodeKind::File } else { NodeKind::Dir },
                reads: 0,
                writes: 0,
                file_touches: Vec::new(),
                children: Vec::new(),
            });
            level.len() - 1
        }
    };

    let node = &mut level[idx];
    // An intermediate segment is always a directory, even if a prior touch
    // first created the node as a leaf file.
    if !is_last {
        node.kind = NodeKind::Dir;
    }
    match t.kind {
        TouchKind::Read => node.reads += 1,
        TouchKind::Write => node.writes += 1,
    }
    if is_last {
        node.file_touches.push(t.span_ref.clone());
    } else {
        insert(&mut node.children, segs, i + 1, &path, t);
    }
}

fn sort_nodes(level: &mut [TouchNode]) {
    level.sort_by(|a, b| {
        // Directories (is_dir == true) sort before files.
        b.is_dir()
            .cmp(&a.is_dir())
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    for n in level.iter_mut() {
        sort_nodes(&mut n.children);
    }
}

/// Collect every directory path in the tree (pre-order). Used to seed the
/// open-directory set and to power expand-all / collapse-all.
pub fn dir_paths(tree: &TouchTree) -> Vec<String> {
    let mut out = Vec::new();
    collect_dirs(&tree.root, &mut out);
    out
}

fn collect_dirs(nodes: &[TouchNode], out: &mut Vec<String>) {
    for n in nodes {
        if n.is_dir() {
            out.push(n.path.clone());
            collect_dirs(&n.children, out);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tref(id: &str) -> TouchRef {
        TouchRef {
            span_id: id.into(),
            trace_id: "t".into(),
            span_pk: 1,
        }
    }

    fn touch(path: &str, kind: TouchKind) -> Touch {
        Touch {
            path: path.into(),
            kind,
            span_ref: tref(path),
        }
    }

    fn find<'a>(nodes: &'a [TouchNode], name: &str) -> &'a TouchNode {
        nodes.iter().find(|n| n.name == name).expect("node present")
    }

    #[test]
    fn empty_touches_yield_empty_tree() {
        let tree = build_tree(&[]);
        assert!(tree.root.is_empty());
    }

    #[test]
    fn single_touch_one_dir_one_file() {
        let tree = build_tree(&[touch("src/main.rs", TouchKind::Write)]);
        assert_eq!(tree.root.len(), 1);
        let src = find(&tree.root, "src");
        assert_eq!(src.kind, NodeKind::Dir);
        assert_eq!(src.writes, 1);
        assert_eq!(src.reads, 0);
        assert_eq!(src.children.len(), 1);
        let main = find(&src.children, "main.rs");
        assert_eq!(main.kind, NodeKind::File);
        assert_eq!(main.writes, 1);
        assert_eq!(main.file_touches.len(), 1);
        assert_eq!(main.path, "src/main.rs");
    }

    #[test]
    fn counts_aggregate_at_every_ancestor() {
        let touches = vec![
            touch("src/a.rs", TouchKind::Read),
            touch("src/b.rs", TouchKind::Write),
            touch("src/tui/x.rs", TouchKind::Write),
        ];
        let tree = build_tree(&touches);
        let src = find(&tree.root, "src");
        // src sees all three touches.
        assert_eq!(src.reads, 1);
        assert_eq!(src.writes, 2);
        let tui = find(&src.children, "tui");
        assert_eq!(tui.writes, 1);
        assert_eq!(tui.reads, 0);
        let x = find(&tui.children, "x.rs");
        assert_eq!(x.writes, 1);
    }

    #[test]
    fn independent_subtrees_have_independent_counts() {
        let touches = vec![
            touch("a/one.rs", TouchKind::Read),
            touch("b/two.rs", TouchKind::Write),
        ];
        let tree = build_tree(&touches);
        let a = find(&tree.root, "a");
        let b = find(&tree.root, "b");
        assert_eq!(a.reads, 1);
        assert_eq!(a.writes, 0);
        assert_eq!(b.reads, 0);
        assert_eq!(b.writes, 1);
    }

    #[test]
    fn identical_touches_accumulate() {
        let touches: Vec<Touch> = (0..3)
            .map(|_| touch("src/main.rs", TouchKind::Write))
            .collect();
        let tree = build_tree(&touches);
        let src = find(&tree.root, "src");
        assert_eq!(src.writes, 3);
        let main = find(&src.children, "main.rs");
        assert_eq!(main.writes, 3);
        assert_eq!(main.file_touches.len(), 3);
    }

    #[test]
    fn trailing_slash_and_repeated_separators_normalized() {
        let touches = vec![
            touch("src//main.rs", TouchKind::Write),
            touch("/src/lib.rs/", TouchKind::Read),
        ];
        let tree = build_tree(&touches);
        // Only one "src" node despite leading/duplicate/trailing slashes.
        assert_eq!(tree.root.len(), 1);
        let src = find(&tree.root, "src");
        assert_eq!(src.children.len(), 2);
        let main = find(&src.children, "main.rs");
        assert_eq!(main.writes, 1);
        let lib = find(&src.children, "lib.rs");
        assert_eq!(lib.reads, 1);
    }

    #[test]
    fn empty_path_is_skipped() {
        let tree = build_tree(&[touch("///", TouchKind::Write), touch("", TouchKind::Read)]);
        assert!(tree.root.is_empty());
    }

    #[test]
    fn sort_dirs_before_files_then_case_insensitive_alpha() {
        let touches = vec![
            touch("Zebra.txt", TouchKind::Read),
            touch("apple.txt", TouchKind::Read),
            touch("Beta/inner.rs", TouchKind::Read),
            touch("alpha/inner.rs", TouchKind::Read),
        ];
        let tree = build_tree(&touches);
        let names: Vec<&str> = tree.root.iter().map(|n| n.name.as_str()).collect();
        // Dirs first (alpha, Beta — case-insensitive), then files (apple, Zebra).
        assert_eq!(names, vec!["alpha", "Beta", "apple.txt", "Zebra.txt"]);
    }

    #[test]
    fn dir_paths_lists_only_directories_preorder() {
        let touches = vec![
            touch("src/tui/x.rs", TouchKind::Write),
            touch("top.rs", TouchKind::Read),
        ];
        let tree = build_tree(&touches);
        let dirs = dir_paths(&tree);
        assert_eq!(dirs, vec!["src".to_string(), "src/tui".to_string()]);
    }
}
