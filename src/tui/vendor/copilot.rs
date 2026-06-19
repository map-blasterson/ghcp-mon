//! Copilot tool-call adapter — pure data transforms specified by the
//! `copilot/llr/Copilot tool-*` LLR family. No I/O, no allocations beyond
//! return values.

use serde_json::Value;

/// Normalized tool kinds (a small closed set the rest of the TUI dispatches
/// on). Other vendors will add their own as they're ported.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolKind {
    Read,
    Edit,
    Write,
    Patch,
    Shell,
}

/// Normalized concept keys for `resolve_argument`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArgConcept {
    FilePath,
    OldText,
    NewText,
    BodyText,
    ShellCommand,
    TargetUrl,
    PatchText,
}

/// Normalized result envelope. All four fields are optional; downstream
/// renderers fall back to the generic-JSON branch when every field is None.
#[derive(Debug, Default, Clone)]
pub struct ResultEnvelope {
    pub body_string: Option<String>,
    pub output_text: Option<String>,
    pub diff_text: Option<String>,
    pub metadata: Option<Value>,
}

/// Per `Copilot tool-name mapping`: `view → read`, `edit → edit`, `create →
/// write`, `apply_patch → patch`, `bash → shell`. Anything else returns None.
pub fn tool_name_mapping(raw_name: &str) -> Option<ToolKind> {
    Some(match raw_name {
        "view" => ToolKind::Read,
        "edit" => ToolKind::Edit,
        "create" => ToolKind::Write,
        "apply_patch" => ToolKind::Patch,
        "bash" => ToolKind::Shell,
        _ => return None,
    })
}

/// Per `Copilot tool-call argument mapping`. `args` is the parsed
/// `gen_ai.tool.call.arguments` JSON value. `PatchText` has a three-step
/// resolution: `args.patch` (string), then `args.input` (string), then `args`
/// itself if it's a string.
pub fn resolve_argument<'a>(args: &'a Value, concept: ArgConcept) -> Option<&'a Value> {
    fn get_str<'a>(v: &'a Value, key: &str) -> Option<&'a Value> {
        v.get(key).filter(|v| v.is_string())
    }
    match concept {
        ArgConcept::FilePath => get_str(args, "path"),
        ArgConcept::OldText => get_str(args, "old_str"),
        ArgConcept::NewText => get_str(args, "new_str"),
        ArgConcept::BodyText => get_str(args, "file_text"),
        ArgConcept::ShellCommand => get_str(args, "command"),
        ArgConcept::TargetUrl => get_str(args, "url"),
        ArgConcept::PatchText => get_str(args, "patch")
            .or_else(|| get_str(args, "input"))
            .or(if args.is_string() { Some(args) } else { None }),
    }
}

/// Per `Copilot tool-call result envelope`: a string raw-result becomes
/// `body_string`; anything else leaves every field unset.
pub fn result_envelope(raw_result: &Value) -> ResultEnvelope {
    let mut env = ResultEnvelope::default();
    if let Value::String(s) = raw_result {
        env.body_string = Some(s.clone());
    }
    env
}

/// Per `Copilot apply_patch path extraction`. Accepts either a raw string
/// (the patch itself) or an object with a `patch`/`input` string property,
/// scans each line for `*** (Add|Update|Delete) File: <path>` or
/// `Move to: <path>`, and returns the distinct paths in first-seen order.
pub fn extract_apply_patch_paths(args: &Value) -> Vec<String> {
    let patch_text: Option<&str> = match args {
        Value::String(s) => Some(s.as_str()),
        Value::Object(map) => map
            .get("patch")
            .and_then(Value::as_str)
            .or_else(|| map.get("input").and_then(Value::as_str)),
        _ => None,
    };
    let Some(text) = patch_text else {
        return Vec::new();
    };

    let mut out: Vec<String> = Vec::new();
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    for line in text.lines() {
        let trimmed = line.trim_start();
        let path = if let Some(rest) = trimmed.strip_prefix("*** ") {
            // "Add File: <path>" / "Update File: <path>" / "Delete File: <path>"
            for verb in ["Add File: ", "Update File: ", "Delete File: "] {
                if let Some(p) = rest.strip_prefix(verb) {
                    let p = p.trim().to_string();
                    if !p.is_empty() && seen.insert(p.clone()) {
                        out.push(p);
                    }
                    break;
                }
            }
            None
        } else if let Some(p) = trimmed.strip_prefix("Move to: ") {
            Some(p.trim().to_string())
        } else {
            None
        };
        if let Some(p) = path {
            if !p.is_empty() && seen.insert(p.clone()) {
                out.push(p);
            }
        }
    }
    out
}

/// Per `Copilot apply_patch diff-stat parsing`. `+++`/`---` markers are
/// header lines and excluded; `+`/`-` prefixed lines are counted as
/// added/removed. Absent patch text yields `(0, 0)`.
pub fn apply_patch_diff_stat(patch_text: Option<&str>) -> (u32, u32) {
    let Some(text) = patch_text else {
        return (0, 0);
    };
    let mut added = 0u32;
    let mut removed = 0u32;
    for line in text.lines() {
        if line.starts_with("+++") || line.starts_with("---") {
            continue;
        }
        if line.starts_with('+') {
            added += 1;
        } else if line.starts_with('-') {
            removed += 1;
        }
    }
    (added, removed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn tool_name_mapping_table() {
        assert_eq!(tool_name_mapping("view"), Some(ToolKind::Read));
        assert_eq!(tool_name_mapping("edit"), Some(ToolKind::Edit));
        assert_eq!(tool_name_mapping("create"), Some(ToolKind::Write));
        assert_eq!(tool_name_mapping("apply_patch"), Some(ToolKind::Patch));
        assert_eq!(tool_name_mapping("bash"), Some(ToolKind::Shell));
        assert_eq!(tool_name_mapping("unknown_tool"), None);
        assert_eq!(tool_name_mapping(""), None);
    }

    #[test]
    fn resolve_argument_simple_concepts() {
        let args = json!({
            "path": "/x/y.rs",
            "old_str": "a",
            "new_str": "b",
            "file_text": "hello",
            "command": "ls -la",
            "url": "https://example.com",
        });
        assert_eq!(
            resolve_argument(&args, ArgConcept::FilePath).unwrap(),
            &json!("/x/y.rs")
        );
        assert_eq!(
            resolve_argument(&args, ArgConcept::OldText).unwrap(),
            &json!("a")
        );
        assert_eq!(
            resolve_argument(&args, ArgConcept::NewText).unwrap(),
            &json!("b")
        );
        assert_eq!(
            resolve_argument(&args, ArgConcept::BodyText).unwrap(),
            &json!("hello")
        );
        assert_eq!(
            resolve_argument(&args, ArgConcept::ShellCommand).unwrap(),
            &json!("ls -la")
        );
        assert_eq!(
            resolve_argument(&args, ArgConcept::TargetUrl).unwrap(),
            &json!("https://example.com")
        );
        // Non-string `path` is rejected.
        let args2 = json!({"path": 7});
        assert_eq!(resolve_argument(&args2, ArgConcept::FilePath), None);
    }

    #[test]
    fn resolve_argument_patch_text_three_step() {
        // Step 1: object.patch
        let a = json!({"patch": "PATCH"});
        assert_eq!(
            resolve_argument(&a, ArgConcept::PatchText).unwrap(),
            &json!("PATCH")
        );
        // Step 2: object.input
        let b = json!({"input": "INPUT"});
        assert_eq!(
            resolve_argument(&b, ArgConcept::PatchText).unwrap(),
            &json!("INPUT")
        );
        // Step 3: args itself a string
        let c = json!("RAW");
        assert_eq!(
            resolve_argument(&c, ArgConcept::PatchText).unwrap(),
            &json!("RAW")
        );
        // Object with neither and not a string → None
        let d = json!({"path": "x"});
        assert_eq!(resolve_argument(&d, ArgConcept::PatchText), None);
    }

    #[test]
    fn result_envelope_string_only() {
        let env = result_envelope(&json!("hello world"));
        assert_eq!(env.body_string.as_deref(), Some("hello world"));
        assert!(env.diff_text.is_none());
        assert!(env.output_text.is_none());
        assert!(env.metadata.is_none());

        let env = result_envelope(&json!({"a": 1}));
        assert!(env.body_string.is_none());
        let env = result_envelope(&json!(null));
        assert!(env.body_string.is_none());
        let env = result_envelope(&json!([1, 2]));
        assert!(env.body_string.is_none());
    }

    #[test]
    fn extract_apply_patch_paths_object_with_patch() {
        let args = json!({"patch": "*** Add File: a.rs\n*** Update File: b.rs\nMove to: c.rs\n"});
        let paths = extract_apply_patch_paths(&args);
        assert_eq!(paths, vec!["a.rs", "b.rs", "c.rs"]);
    }

    #[test]
    fn extract_apply_patch_paths_raw_string_and_dedupe() {
        let args = json!("*** Add File: x.rs\n*** Add File: x.rs\n*** Delete File: y.rs\n");
        let paths = extract_apply_patch_paths(&args);
        assert_eq!(paths, vec!["x.rs", "y.rs"]);
    }

    #[test]
    fn extract_apply_patch_paths_falls_through_to_input() {
        let args = json!({"input": "*** Add File: z.rs\n"});
        assert_eq!(extract_apply_patch_paths(&args), vec!["z.rs"]);
    }

    #[test]
    fn extract_apply_patch_paths_no_matches() {
        assert!(extract_apply_patch_paths(&json!("nothing here")).is_empty());
        assert!(extract_apply_patch_paths(&json!(42)).is_empty());
        assert!(extract_apply_patch_paths(&json!({"patch": 5})).is_empty());
    }

    #[test]
    fn diff_stat_skips_headers_and_counts() {
        let p = "--- old.rs\n+++ new.rs\n@@ -1,3 +1,4 @@\n-line a\n line b\n+line c\n+line d\n";
        assert_eq!(apply_patch_diff_stat(Some(p)), (2, 1));
    }

    #[test]
    fn diff_stat_none_zero() {
        assert_eq!(apply_patch_diff_stat(None), (0, 0));
        assert_eq!(apply_patch_diff_stat(Some("")), (0, 0));
    }
}
