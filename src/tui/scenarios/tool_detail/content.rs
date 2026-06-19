//! Span-attribute content helpers specific to the tool-detail renderers.
//! [`parse_tool_call_arguments`] is reused verbatim from
//! [`crate::tui::scenarios::spans::attrs`]; this module adds the result
//! counterpart and small object/null helpers used across the renderers.
//!
//! Source for (shared `frontend/llr/`):
//! - `Generic tool renders args splitting code-ish strings`
//! - `Edit tool result renders unified diff from metadata`
//! - `View tool splits line numbers into gutter`

use serde_json::Value;

use crate::tui::scenarios::spans::attrs::attr;

/// Parse `gen_ai.tool.call.result` from a span's attributes. Mirrors web
/// `parseToolCallResult`:
/// - missing key → `None`;
/// - a string that JSON-parses to a non-string value → that parsed value;
/// - a string that fails to parse, or parses to a string → the raw string
///   verbatim (so callers can render it as a single block);
/// - any other inline value → passed through unchanged.
pub fn parse_tool_call_result(attrs: &Value) -> Option<Value> {
    let raw = attr(attrs, "gen_ai.tool.call.result")?;
    if raw.is_null() {
        return None;
    }
    Some(match raw {
        Value::String(s) => match serde_json::from_str::<Value>(s) {
            Ok(v) if !v.is_string() => v,
            _ => Value::String(s.clone()),
        },
        other => other.clone(),
    })
}

/// Borrow `v` as a JSON object iff it is one (not an array, not a scalar).
pub fn as_object(v: &Value) -> Option<&serde_json::Map<String, Value>> {
    v.as_object()
}

/// True when the value is JSON `null`.
pub fn is_nullish(v: &Value) -> bool {
    v.is_null()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn missing_key_is_none() {
        assert!(parse_tool_call_result(&json!({})).is_none());
    }

    #[test]
    fn null_value_is_none() {
        assert!(parse_tool_call_result(&json!({"gen_ai.tool.call.result": null})).is_none());
    }

    #[test]
    fn plain_string_stays_string() {
        let a = json!({"gen_ai.tool.call.result": "stdout\n<exited with exit code 0>"});
        let r = parse_tool_call_result(&a).unwrap();
        assert_eq!(r, json!("stdout\n<exited with exit code 0>"));
    }

    #[test]
    fn json_object_string_parses_through() {
        let a = json!({"gen_ai.tool.call.result": "{\"output\":\"ok\"}"});
        let r = parse_tool_call_result(&a).unwrap();
        assert_eq!(r["output"], "ok");
    }

    #[test]
    fn json_string_literal_stays_raw() {
        // `"\"hello\""` parses to the JSON string "hello"; we keep the raw form.
        let a = json!({"gen_ai.tool.call.result": "\"hello\""});
        let r = parse_tool_call_result(&a).unwrap();
        assert_eq!(r, json!("\"hello\""));
    }

    #[test]
    fn inline_object_passthrough() {
        let a = json!({"gen_ai.tool.call.result": {"output": "ok"}});
        let r = parse_tool_call_result(&a).unwrap();
        assert_eq!(r["output"], "ok");
    }
}
