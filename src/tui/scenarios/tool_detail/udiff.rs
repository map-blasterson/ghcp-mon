//! Unified-diff line classifier — the pure helper behind the `edit`/`write`
//! result renderer's colored diff. Terminal port of the web `udiffClassify`
//! (`web/src/scenarios/ToolDetail.tsx`).
//!
//! Source for (shared `frontend/llr/`):
//! - `Edit tool result renders unified diff from metadata`
//!
//! Also source for (new `frontend/tui/llr/`):
//! - `TUI Udiff classify line precedence`

/// Classification of a single unified-diff line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UdiffLine {
    /// File header (`+++`/`---`).
    Meta,
    /// Hunk header (`@@`).
    Hunk,
    /// Added line (`+`, not a `+++` header).
    Add,
    /// Removed line (`-`, not a `---` header).
    Rem,
    /// Context / unchanged line.
    Line,
}

/// Classify a unified-diff line. Precedence (highest first):
/// `+++`/`---` (Meta) > `@@` (Hunk) > `+` (Add) > `-` (Rem) > else (Line).
///
/// Mirrors the web `UnifiedDiff` line classifier: the `+++`/`---` file-header
/// test takes precedence over the single-character add/remove test, so a bare
/// `"+++"` classifies as `Meta`, not `Add`.
pub fn udiff_classify(line: &str) -> UdiffLine {
    if line.starts_with("+++") || line.starts_with("---") {
        return UdiffLine::Meta;
    }
    if line.starts_with("@@") {
        return UdiffLine::Hunk;
    }
    if line.starts_with("Index:") || line.starts_with("====") {
        return UdiffLine::Meta;
    }
    match line.as_bytes().first().copied() {
        Some(b'+') => UdiffLine::Add,
        Some(b'-') => UdiffLine::Rem,
        _ => UdiffLine::Line,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn meta_file_headers() {
        assert_eq!(udiff_classify("+++ b/file.rs"), UdiffLine::Meta);
        assert_eq!(udiff_classify("--- a/file"), UdiffLine::Meta);
    }

    #[test]
    fn hunk_header() {
        assert_eq!(udiff_classify("@@ -1,3 +1,4 @@"), UdiffLine::Hunk);
        assert_eq!(udiff_classify("@@@@"), UdiffLine::Hunk);
    }

    #[test]
    fn add_and_rem() {
        assert_eq!(udiff_classify("+added line"), UdiffLine::Add);
        assert_eq!(udiff_classify("-removed line"), UdiffLine::Rem);
    }

    #[test]
    fn context_line() {
        assert_eq!(udiff_classify(" context"), UdiffLine::Line);
        assert_eq!(udiff_classify("no marker"), UdiffLine::Line);
    }

    #[test]
    fn edge_cases() {
        // `+++` alone classifies as Meta (header test wins over Add).
        assert_eq!(udiff_classify("+++"), UdiffLine::Meta);
        // `+` alone → Add.
        assert_eq!(udiff_classify("+"), UdiffLine::Add);
        // empty string → Line.
        assert_eq!(udiff_classify(""), UdiffLine::Line);
        // `@@@@` → Hunk.
        assert_eq!(udiff_classify("@@@@"), UdiffLine::Hunk);
        // `--- a/file` → Meta.
        assert_eq!(udiff_classify("--- a/file"), UdiffLine::Meta);
    }
}
