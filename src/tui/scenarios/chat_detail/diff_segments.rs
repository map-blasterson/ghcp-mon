//! Word-level diff segments for DELTA mode.
//!
//! Backed by [`similar::TextDiff::from_words`], which segments the two inputs
//! into word tokens (preserving whitespace) and aligns them with Myers' diff.
//! The output is a flat `Vec<DiffSegment>` interleaving Unchanged/Added/Removed
//! runs that the renderer overlays with REVERSED add/rem styling.
//!
//! Source for (shared `frontend/llr/`):
//! - `Chat detail system instructions word-diff in DELTA`

use similar::{ChangeTag, TextDiff};

/// One contiguous word-aligned segment of the diff.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiffSegment {
    /// Identical between prior and current.
    Unchanged(String),
    /// Present only in current — render with `+` background.
    Added(String),
    /// Present only in prior — render with `-` background.
    Removed(String),
}

/// Run word-level diff over `prior` and `current` and return the flat segment
/// list. Consecutive segments of the same tag are merged so the output is the
/// minimal sequence that reconstructs the diff.
pub fn word_diff(prior: &str, current: &str) -> Vec<DiffSegment> {
    let diff = TextDiff::from_words(prior, current);
    let mut out: Vec<DiffSegment> = Vec::new();
    for change in diff.iter_all_changes() {
        let v = change.value().to_string();
        if v.is_empty() {
            continue;
        }
        let new_seg = match change.tag() {
            ChangeTag::Equal => DiffSegment::Unchanged(v),
            ChangeTag::Insert => DiffSegment::Added(v),
            ChangeTag::Delete => DiffSegment::Removed(v),
        };
        match (out.last_mut(), &new_seg) {
            (Some(DiffSegment::Unchanged(prev)), DiffSegment::Unchanged(s)) => prev.push_str(s),
            (Some(DiffSegment::Added(prev)), DiffSegment::Added(s)) => prev.push_str(s),
            (Some(DiffSegment::Removed(prev)), DiffSegment::Removed(s)) => prev.push_str(s),
            _ => out.push(new_seg),
        }
    }
    out
}

/// Total character count of all `Added` segments.
pub fn count_added(segments: &[DiffSegment]) -> usize {
    segments
        .iter()
        .filter_map(|s| match s {
            DiffSegment::Added(t) => Some(t.chars().count()),
            _ => None,
        })
        .sum()
}

/// Total character count of all `Removed` segments.
pub fn count_removed(segments: &[DiffSegment]) -> usize {
    segments
        .iter()
        .filter_map(|s| match s {
            DiffSegment::Removed(t) => Some(t.chars().count()),
            _ => None,
        })
        .sum()
}

/// True iff every segment is `Unchanged`.
pub fn all_unchanged(segments: &[DiffSegment]) -> bool {
    segments.iter().all(|s| matches!(s, DiffSegment::Unchanged(_)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identical_inputs_all_unchanged() {
        let segs = word_diff("hello world", "hello world");
        assert!(all_unchanged(&segs));
        assert_eq!(count_added(&segs), 0);
        assert_eq!(count_removed(&segs), 0);
    }

    #[test]
    fn pure_addition() {
        let segs = word_diff("hello", "hello world");
        assert!(segs.iter().any(|s| matches!(s, DiffSegment::Added(_))));
        assert!(!segs.iter().any(|s| matches!(s, DiffSegment::Removed(_))));
        assert!(count_added(&segs) > 0);
    }

    #[test]
    fn pure_removal() {
        let segs = word_diff("hello world", "hello");
        assert!(segs.iter().any(|s| matches!(s, DiffSegment::Removed(_))));
        assert!(!segs.iter().any(|s| matches!(s, DiffSegment::Added(_))));
        assert!(count_removed(&segs) > 0);
    }

    #[test]
    fn mixed_changes_have_both() {
        let segs = word_diff("the quick brown fox", "the slow brown cat");
        assert!(count_added(&segs) > 0);
        assert!(count_removed(&segs) > 0);
    }

    #[test]
    fn whitespace_is_preserved_in_segments() {
        let segs = word_diff("a b", "a c");
        let reconstructed_current: String = segs
            .iter()
            .filter_map(|s| match s {
                DiffSegment::Unchanged(t) | DiffSegment::Added(t) => Some(t.clone()),
                _ => None,
            })
            .collect();
        assert_eq!(reconstructed_current, "a c");
    }
}
