//! Hero-panel value selection — the pure helper behind
//! `Tool detail hero panel surfaces key argument`. Terminal port of the web
//! `pickHero`.
//!
//! Source for (shared `frontend/llr/`):
//! - `Tool detail hero panel surfaces key argument`

use serde_json::Value;

use crate::tui::format::pretty_json;

/// Priority-ordered argument keys whose value is hoisted into the hero panel.
pub const HERO_KEYS: [&str; 3] = ["command", "query", "description"];

/// Select the hero value for a native tool span.
///
/// Returns `Some(rendered_string)` only when:
/// - `tool_type == Some("function")`, **and**
/// - `args` is a non-null JSON object, **and**
/// - the first present key from [`HERO_KEYS`] has a non-null value whose
///   string rendering (string values verbatim; non-string via
///   [`pretty_json`]) is non-empty.
///
/// Only the first matching priority key is surfaced. Returns `None` in every
/// other case (including external spans — the caller is responsible for never
/// invoking this for `external_tool_call` spans).
pub fn hero_value(args: &Value, tool_type: Option<&str>) -> Option<String> {
    if tool_type != Some("function") {
        return None;
    }
    let obj = args.as_object()?;
    for key in HERO_KEYS {
        let Some(v) = obj.get(key) else {
            continue;
        };
        if v.is_null() {
            continue;
        }
        let sv = match v {
            Value::String(s) => s.clone(),
            other => pretty_json(other),
        };
        if sv.is_empty() {
            continue;
        }
        return Some(sv);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn command_has_priority_over_others() {
        let args = json!({"description": "d", "command": "ls -la", "query": "q"});
        assert_eq!(hero_value(&args, Some("function")).as_deref(), Some("ls -la"));
    }

    #[test]
    fn query_when_no_command() {
        let args = json!({"description": "d", "query": "search me"});
        assert_eq!(
            hero_value(&args, Some("function")).as_deref(),
            Some("search me")
        );
    }

    #[test]
    fn description_when_no_command_or_query() {
        let args = json!({"description": "do the thing"});
        assert_eq!(
            hero_value(&args, Some("function")).as_deref(),
            Some("do the thing")
        );
    }

    #[test]
    fn non_function_tool_type_suppresses() {
        let args = json!({"command": "ls"});
        assert_eq!(hero_value(&args, Some("mcp")), None);
        assert_eq!(hero_value(&args, None), None);
    }

    #[test]
    fn null_args_object_suppresses() {
        assert_eq!(hero_value(&Value::Null, Some("function")), None);
        // Non-object (array) also suppresses.
        assert_eq!(hero_value(&json!([1, 2]), Some("function")), None);
    }

    #[test]
    fn missing_priority_keys_suppress() {
        let args = json!({"path": "/x", "limit": 5});
        assert_eq!(hero_value(&args, Some("function")), None);
    }

    #[test]
    fn null_value_skipped_then_next_key_used() {
        let args = json!({"command": null, "query": "fallback"});
        assert_eq!(
            hero_value(&args, Some("function")).as_deref(),
            Some("fallback")
        );
    }

    #[test]
    fn empty_string_value_skipped() {
        let args = json!({"command": "", "query": "q"});
        assert_eq!(hero_value(&args, Some("function")).as_deref(), Some("q"));
        // All empty/absent → None.
        let args2 = json!({"command": ""});
        assert_eq!(hero_value(&args2, Some("function")), None);
    }

    #[test]
    fn non_string_value_via_pretty_json() {
        let args = json!({"command": {"shell": "bash"}});
        let v = hero_value(&args, Some("function")).unwrap();
        assert!(v.contains("\"shell\""), "got: {v:?}");
    }
}
