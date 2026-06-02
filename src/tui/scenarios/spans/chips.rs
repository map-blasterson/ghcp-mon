//! Inline-row chip extractors for the Spans tree.
//!
//! Implements:
//! - `Shell command chip extracts primary words`
//! - `Skill name chip shows skill argument`
//! - `Report intent title shows on parent row`
//! - `Spans tool description inline label`
//! - `Spans target badge shows file basename or URL domain`
//! - `Spans diff stat badges on file mutation tools`

use serde_json::Value;

use crate::tui::vendor::copilot::{ToolKind, apply_patch_diff_stat, extract_apply_patch_paths};

const SHELL_CHIP_TRUNCATE: usize = 24;
const SHELL_CHIP_MAX: usize = 6;

/// Split a shell command into up to 6 primary-word chips + a `…` overflow.
/// Per `Shell command chip extracts primary words`.
pub fn shell_command_chips(cmd: &str) -> Vec<String> {
    if cmd.trim().is_empty() {
        return Vec::new();
    }
    // Split on whitespace-bounded `&&`, `||`, or `|`.
    let segments = split_pipeline(cmd);
    let mut chips: Vec<String> = Vec::new();
    for seg in &segments {
        // First non-`KEY=value` token.
        let tok = first_non_env_token(seg);
        let Some(tok) = tok else { continue };
        let basename = basename(&tok);
        let truncated = truncate_chars(&basename, SHELL_CHIP_TRUNCATE);
        chips.push(truncated);
        if chips.len() == SHELL_CHIP_MAX && segments.len() > SHELL_CHIP_MAX {
            chips.push("…".to_string());
            break;
        }
    }
    chips
}

fn split_pipeline(s: &str) -> Vec<String> {
    // Walk char-by-char; split when we see ` && `, ` || `, or ` | `.
    let mut out: Vec<String> = Vec::new();
    let mut cur = String::new();
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let rest = &s[i..];
        if rest.starts_with(" && ") {
            out.push(std::mem::take(&mut cur));
            i += 4;
            continue;
        }
        if rest.starts_with(" || ") {
            out.push(std::mem::take(&mut cur));
            i += 4;
            continue;
        }
        if rest.starts_with(" | ") {
            out.push(std::mem::take(&mut cur));
            i += 3;
            continue;
        }
        // Push one char (handle utf-8 properly).
        let c = rest.chars().next().unwrap();
        cur.push(c);
        i += c.len_utf8();
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

fn first_non_env_token(seg: &str) -> Option<String> {
    for tok in seg.split_ascii_whitespace() {
        // KEY=value pattern: starts with an uppercase ascii letter or `_`,
        // contains an `=`, and the part before `=` is `[A-Z0-9_]+`.
        if let Some(eq_idx) = tok.find('=') {
            let key = &tok[..eq_idx];
            if !key.is_empty()
                && key
                    .chars()
                    .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_')
            {
                continue;
            }
        }
        return Some(tok.to_string());
    }
    None
}

fn basename(p: &str) -> String {
    match p.rsplit_once('/') {
        Some((_, name)) if !name.is_empty() => name.to_string(),
        _ => p.to_string(),
    }
}

fn truncate_chars(s: &str, max: usize) -> String {
    let count = s.chars().count();
    if count <= max {
        return s.to_string();
    }
    let mut out: String = s.chars().take(max.saturating_sub(1)).collect();
    out.push('…');
    out
}

/// Per `Skill name chip shows skill argument`.
pub fn skill_chip(args: &Value) -> Option<String> {
    if !args.is_object() || args.is_array() {
        return None;
    }
    let s = args.get("skill")?.as_str()?;
    if s.is_empty() {
        return None;
    }
    Some(s.to_string())
}

/// Per `Report intent title shows on parent row`.
pub fn report_intent_title(args: &Value) -> Option<String> {
    if !args.is_object() || args.is_array() {
        return None;
    }
    let s = args.get("intent")?.as_str()?;
    if s.is_empty() {
        return None;
    }
    Some(s.to_string())
}

/// Per `Spans tool description inline label`.
pub fn tool_description_label(args: &Value) -> Option<String> {
    let s = args.get("description")?.as_str()?;
    if s.is_empty() {
        return None;
    }
    Some(s.to_string())
}

/// Inline content preview for a Chat-kind span row. Looks at the chat's
/// captured messages and returns the first text it can show, normalised to
/// a single line. Preference order: the assistant's reply (last text part
/// across `gen_ai.output.messages`), then the most recent input message's
/// text part (typically the user prompt). Returns `None` when nothing
/// previewable exists (e.g. tool-call-only chats, no captured content).
///
/// The Spans row renderer (see `SpansTreeRow` in `widgets/spans_tree_row.rs`)
/// truncates to whatever fits the column width and appends `…` — this
/// helper does no length capping of its own.
pub fn chat_text_preview(attrs: &Value) -> Option<String> {
    use crate::tui::scenarios::chat_detail::messages::{
        parse_input_messages, parse_output_messages, Part,
    };

    let pick_text = |msgs: &[crate::tui::scenarios::chat_detail::messages::Message]| -> Option<String> {
        for m in msgs {
            for p in &m.parts {
                if let Part::Text { content, .. } = p {
                    let trimmed = content.trim();
                    if !trimmed.is_empty() {
                        return Some(trimmed.to_string());
                    }
                }
            }
        }
        None
    };

    let raw = pick_text(&parse_output_messages(attrs))
        .or_else(|| {
            // Fall back to the LAST input message — the freshest user
            // prompt — rather than the first (which on a multi-turn
            // session is the long stale system primer).
            let mut msgs = parse_input_messages(attrs);
            msgs.reverse();
            pick_text(&msgs)
        })?;

    // Collapse all internal whitespace runs to single spaces — the row is
    // one cell tall so any embedded newline would otherwise blow the
    // budget and corrupt the layout downstream.
    let single_line: String =
        raw.split_whitespace().collect::<Vec<&str>>().join(" ");
    if single_line.is_empty() {
        None
    } else {
        Some(single_line)
    }
}

/// Per `Spans target badge shows file basename or URL domain`.
pub fn target_chips(kind: Option<ToolKind>, args: &Value) -> Vec<String> {
    if !args.is_object() || args.is_array() {
        return Vec::new();
    }
    if let Some(path) = args
        .get("path")
        .or_else(|| args.get("filePath"))
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
    {
        return vec![target_basename(path)];
    }
    if matches!(kind, Some(ToolKind::Patch)) {
        let paths = extract_apply_patch_paths(args);
        if !paths.is_empty() {
            return paths.into_iter().map(|p| target_basename(&p)).collect();
        }
    }
    args.get("url")
        .or_else(|| args.get("target-url"))
        .or_else(|| args.get("target_url"))
        .and_then(Value::as_str)
        .and_then(url_hostname)
        .into_iter()
        .collect()
}

fn target_basename(p: &str) -> String {
    let is_windows = p.len() >= 3
        && p.as_bytes()[0].is_ascii_alphabetic()
        && p.as_bytes()[1] == b':'
        && matches!(p.as_bytes()[2], b'\\' | b'/');
    if is_windows {
        p.rsplit(['\\', '/'])
            .next()
            .filter(|s| !s.is_empty())
            .unwrap_or(p)
            .to_string()
    } else {
        p.rsplit('/')
            .next()
            .filter(|s| !s.is_empty())
            .unwrap_or(p)
            .to_string()
    }
}

fn url_hostname(url: &str) -> Option<String> {
    let (_, rest) = url.split_once("://")?;
    let authority = rest.split(['/', '?', '#']).next().unwrap_or_default();
    let host_port = authority.rsplit('@').next().unwrap_or_default();
    let host = if let Some(end) = host_port
        .strip_prefix('[')
        .and_then(|s| s.find(']').map(|i| &s[..i]))
    {
        end
    } else {
        host_port.split(':').next().unwrap_or_default()
    };
    (!host.is_empty()).then(|| host.to_string())
}

/// Per `Spans diff stat badges on file mutation tools`. Returns
/// `(added, removed)`.
///
/// For `Edit`, runs the same line-level diff used by the inline-diff
/// renderer ([`crate::tui::scenarios::tool_detail::inline_diff::build`])
/// and counts the diff rows that are actually added or removed — NOT the
/// total line counts of `old_str` and `new_str`. The latter is what the
/// previous implementation did and it over-counted a one-line touch in a
/// many-line block as `+N -N`.
pub fn diff_stat(kind: ToolKind, args: &Value) -> (u32, u32) {
    use crate::tui::scenarios::tool_detail::inline_diff;
    match kind {
        ToolKind::Edit => {
            let old = args.get("old_str").and_then(Value::as_str).unwrap_or("");
            let new = args.get("new_str").and_then(Value::as_str).unwrap_or("");
            inline_diff::count_changes(&inline_diff::build(old, new))
        }
        ToolKind::Write => {
            let body = args.get("file_text").and_then(Value::as_str).unwrap_or("");
            (count_lines(body), 0)
        }
        ToolKind::Patch => {
            let txt = args
                .get("patch")
                .and_then(Value::as_str)
                .or_else(|| args.get("input").and_then(Value::as_str))
                .or_else(|| args.as_str());
            apply_patch_diff_stat(txt)
        }
        _ => (0, 0),
    }
}

/// Per `count_lines` semantics in the diff-stat LLR. A trailing `\n` is a
/// terminator (`"foo\n"` = 1 line); empty string is 0 lines.
pub fn count_lines(s: &str) -> u32 {
    if s.is_empty() {
        return 0;
    }
    let mut n = s.matches('\n').count() as u32;
    if !s.ends_with('\n') {
        n += 1;
    }
    n
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn shell_chip_empty() {
        assert!(shell_command_chips("").is_empty());
        assert!(shell_command_chips("   ").is_empty());
    }

    #[test]
    fn shell_chip_single_command() {
        assert_eq!(shell_command_chips("ls -la"), vec!["ls"]);
    }

    #[test]
    fn shell_chip_pipeline_splits() {
        let v = shell_command_chips("ls -la | grep foo && echo ok");
        assert_eq!(v, vec!["ls", "grep", "echo"]);
    }

    #[test]
    fn shell_chip_or_split() {
        let v = shell_command_chips("foo || bar");
        assert_eq!(v, vec!["foo", "bar"]);
    }

    #[test]
    fn shell_chip_skips_env_assignment() {
        let v = shell_command_chips("FOO=bar BAR_BAZ=1 /usr/bin/python script.py");
        assert_eq!(v, vec!["python"]);
    }

    #[test]
    fn shell_chip_basename_and_truncate() {
        let v = shell_command_chips("/very/long/path/to/an/executable_with_a_long_name --flag");
        assert_eq!(v.len(), 1);
        // truncated to 24 chars including ellipsis
        assert!(v[0].chars().count() <= 24);
    }

    #[test]
    fn shell_chip_caps_at_six_with_overflow() {
        // 7 segments -> 6 chips + …
        let cmd = "a | b | c | d | e | f | g";
        let v = shell_command_chips(cmd);
        assert_eq!(v.len(), 7);
        assert_eq!(v.last().unwrap(), "…");
    }

    #[test]
    fn skill_chip_present_and_missing() {
        assert_eq!(
            skill_chip(&json!({"skill": "summarize"})).as_deref(),
            Some("summarize")
        );
        assert!(skill_chip(&json!({})).is_none());
        assert!(skill_chip(&json!({"skill": ""})).is_none());
        assert!(skill_chip(&json!([1, 2])).is_none());
        assert!(skill_chip(&json!("notobj")).is_none());
    }

    #[test]
    fn report_intent_title_present_and_missing() {
        assert_eq!(
            report_intent_title(&json!({"intent": "rewrite"})).as_deref(),
            Some("rewrite")
        );
        assert!(report_intent_title(&json!({"intent": ""})).is_none());
        assert!(report_intent_title(&json!({})).is_none());
    }

    #[test]
    fn tool_description_label_present_and_missing() {
        assert_eq!(
            tool_description_label(&json!({"description": "do thing"})).as_deref(),
            Some("do thing")
        );
        assert!(tool_description_label(&json!({"description": ""})).is_none());
        assert!(tool_description_label(&json!({})).is_none());
    }

    #[test]
    fn target_chips_path_basename() {
        assert_eq!(
            target_chips(Some(ToolKind::Edit), &json!({"path": "/repo/src/lib.rs"})),
            vec!["lib.rs"]
        );
        assert_eq!(
            target_chips(Some(ToolKind::Write), &json!({"filePath": "C:\\work\\main.rs"})),
            vec!["main.rs"]
        );
    }

    #[test]
    fn target_chips_url_hostname() {
        assert_eq!(
            target_chips(None, &json!({"url": "https://example.com/a/b"})),
            vec!["example.com"]
        );
        assert!(target_chips(None, &json!({"url": "not a url"})).is_empty());
    }

    #[test]
    fn target_chips_patch_paths() {
        let args = json!({"patch": "*** Add File: src/a.rs\n*** Update File: crates/b.rs\n"});
        assert_eq!(
            target_chips(Some(ToolKind::Patch), &args),
            vec!["a.rs", "b.rs"]
        );
    }

    #[test]
    fn target_chips_rejects_non_object() {
        assert!(target_chips(Some(ToolKind::Read), &json!("/repo/src/lib.rs")).is_empty());
        assert!(target_chips(Some(ToolKind::Read), &json!([])).is_empty());
    }

    #[test]
    fn count_lines_semantics() {
        assert_eq!(count_lines(""), 0);
        assert_eq!(count_lines("foo"), 1);
        assert_eq!(count_lines("foo\n"), 1);
        assert_eq!(count_lines("foo\nbar"), 2);
        assert_eq!(count_lines("foo\nbar\n"), 2);
        assert_eq!(count_lines("\n"), 1);
    }

    #[test]
    fn diff_stat_edit() {
        let args = json!({"old_str": "a\nb\n", "new_str": "x\n"});
        assert_eq!(diff_stat(ToolKind::Edit, &args), (1, 2));
    }

    #[test]
    fn diff_stat_write() {
        let args = json!({"file_text": "line1\nline2\nline3"});
        assert_eq!(diff_stat(ToolKind::Write, &args), (3, 0));
    }

    #[test]
    fn diff_stat_patch_uses_vendor() {
        let args = json!({"patch": "--- a\n+++ b\n@@\n+new\n-old\n"});
        let (added, removed) = diff_stat(ToolKind::Patch, &args);
        assert_eq!(added, 1);
        assert_eq!(removed, 1);
    }

    #[test]
    fn diff_stat_other_kinds_zero() {
        assert_eq!(diff_stat(ToolKind::Read, &json!({})), (0, 0));
        assert_eq!(diff_stat(ToolKind::Shell, &json!({})), (0, 0));
    }

    /// Regression for bug "+/- chip counts are bogus": the prior
    /// implementation returned (count_lines(new), count_lines(old)),
    /// which over-counts a one-line touch in a many-line block as `+N -N`.
    /// The fix routes through the real line-level diff so a one-line
    /// change reports `(1, 1)`.
    #[test]
    fn diff_stat_edit_counts_actual_changed_lines_not_block_totals() {
        let args = json!({
            "old_str": "fn a() {\n    let x = 1;\n    println!(\"x\");\n    return;\n}\n",
            "new_str": "fn a() {\n    let x = 2;\n    println!(\"x\");\n    return;\n}\n",
        });
        assert_eq!(diff_stat(ToolKind::Edit, &args), (1, 1));
    }

    #[test]
    fn chat_text_preview_prefers_output_message_text() {
        let attrs = json!({
            "gen_ai.input.messages": [
                {"role": "user", "parts": [{"type": "text", "content": "ask"}]}
            ],
            "gen_ai.output.messages": [
                {"role": "assistant", "parts": [{"type": "text", "content": "Hello there"}]}
            ],
        });
        assert_eq!(chat_text_preview(&attrs).as_deref(), Some("Hello there"));
    }

    #[test]
    fn chat_text_preview_falls_back_to_last_input_when_no_output() {
        let attrs = json!({
            "gen_ai.input.messages": [
                {"role": "system", "parts": [{"type": "text", "content": "stale primer"}]},
                {"role": "user", "parts": [{"type": "text", "content": "fresh question"}]}
            ],
            "gen_ai.output.messages": [],
        });
        // Falls back to the LAST input message, not the first stale system primer.
        assert_eq!(chat_text_preview(&attrs).as_deref(), Some("fresh question"));
    }

    #[test]
    fn chat_text_preview_returns_full_text() {
        // The renderer is the only thing that knows the column width and
        // applies truncation+ellipsis there; the helper returns the
        // single-line normalised text verbatim.
        let long = "a".repeat(10_000);
        let attrs = json!({
            "gen_ai.output.messages": [
                {"role": "assistant", "parts": [{"type": "text", "content": long}]}
            ],
        });
        let p = chat_text_preview(&attrs).unwrap();
        assert_eq!(p.chars().count(), 10_000);
        assert!(!p.contains('…'));
    }

    #[test]
    fn chat_text_preview_collapses_internal_whitespace() {
        let attrs = json!({
            "gen_ai.output.messages": [
                {"role": "assistant", "parts": [{"type": "text", "content": "  hi\n\n there\tworld  "}]}
            ],
        });
        // No embedded newline / tab — the row is one cell tall.
        assert_eq!(chat_text_preview(&attrs).as_deref(), Some("hi there world"));
    }

    #[test]
    fn chat_text_preview_none_for_tool_call_only() {
        let attrs = json!({
            "gen_ai.output.messages": [
                {"role": "assistant", "parts": [{"type": "tool_call", "id": "x", "name": "y"}]}
            ],
        });
        assert!(chat_text_preview(&attrs).is_none());
    }

    #[test]
    fn chat_text_preview_none_for_empty_attrs() {
        assert!(chat_text_preview(&json!({})).is_none());
        assert!(chat_text_preview(&json!({
            "gen_ai.input.messages": [], "gen_ai.output.messages": []
        })).is_none());
    }
}
