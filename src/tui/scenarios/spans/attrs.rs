//! Helpers for reading values out of a `SpanFull.attributes` map. The Copilot
//! and OpenTelemetry GenAI semconv use string keys like
//! `gen_ai.tool.call.arguments` whose values may be either an inline JSON
//! value or a stringified JSON blob.

use serde_json::Value;

/// Look up a single attribute by exact key.
pub fn attr<'a>(attrs: &'a Value, key: &str) -> Option<&'a Value> {
    attrs.as_object()?.get(key)
}

/// Parse `gen_ai.tool.call.arguments` from a span's attributes. Mirrors web
/// `parseToolCallArguments`: accepts either an object/array directly or a
/// string that is itself JSON. Returns `Value::Null` on parse failure (or
/// `None` if the key is missing entirely).
pub fn parse_tool_call_arguments(attrs: &Value) -> Option<Value> {
    let v = attr(attrs, "gen_ai.tool.call.arguments")?;
    Some(match v {
        Value::String(s) if !s.is_empty() => {
            serde_json::from_str(s).unwrap_or(Value::Null)
        }
        Value::String(_) => Value::Null,
        other => other.clone(),
    })
}

/// Read the captured tool name (`gen_ai.tool.name`) from attributes.
pub fn tool_name(attrs: &Value) -> Option<String> {
    attr(attrs, "gen_ai.tool.name")
        .and_then(Value::as_str)
        .map(|s| s.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parses_stringified_args() {
        let attrs = json!({"gen_ai.tool.call.arguments": "{\"command\":\"ls\"}"});
        let v = parse_tool_call_arguments(&attrs).unwrap();
        assert_eq!(v["command"], "ls");
    }

    #[test]
    fn passes_inline_object_through() {
        let attrs = json!({"gen_ai.tool.call.arguments": {"command": "ls"}});
        let v = parse_tool_call_arguments(&attrs).unwrap();
        assert_eq!(v["command"], "ls");
    }

    #[test]
    fn returns_none_for_missing_key() {
        let attrs = json!({});
        assert!(parse_tool_call_arguments(&attrs).is_none());
    }

    #[test]
    fn malformed_string_yields_null() {
        let attrs = json!({"gen_ai.tool.call.arguments": "not json"});
        assert_eq!(parse_tool_call_arguments(&attrs), Some(Value::Null));
    }
}
