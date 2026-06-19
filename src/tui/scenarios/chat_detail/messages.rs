//! Message-part parsing helpers ported from the web `MessageView`.
//!
//! Source for (shared `frontend/llr/`):
//! - `Content attrs accepts object or json string`
//! - `Content parses input output messages`
//! - `Content parses tool call result`
//! - `Content has captured content predicate`
//! - `Message view renders parts by type`

use serde_json::Value;

/// One chat-message envelope from the GenAI semconv input/output arrays.
#[derive(Debug, Clone, PartialEq)]
pub struct Message {
    pub role: String,
    pub parts: Vec<Part>,
    pub finish_reason: Option<String>,
    /// Raw entry value preserved so node-level JSON.length can be computed
    /// without re-encoding the typed fields.
    pub raw: Value,
}

/// One element of `Message::parts`. Mirrors the OTel GenAI part-type
/// discriminator (`text` / `reasoning` / `tool_call` / `tool_call_response`),
/// with an `Other` catch-all that preserves the raw JSON for forward
/// compatibility per the `Message view renders parts by type` LLR.
#[derive(Debug, Clone, PartialEq)]
pub enum Part {
    Text { content: String, raw: Value },
    Reasoning { content: String, raw: Value },
    ToolCall { name: String, id: String, arguments: Value, raw: Value },
    ToolCallResponse { id: String, result: Value, raw: Value },
    Other { raw: Value },
}

impl Part {
    /// Raw underlying JSON; used for both `JSON.stringify(value).len()` byte
    /// computation and search-content extraction.
    pub fn raw(&self) -> &Value {
        match self {
            Part::Text { raw, .. }
            | Part::Reasoning { raw, .. }
            | Part::ToolCall { raw, .. }
            | Part::ToolCallResponse { raw, .. }
            | Part::Other { raw } => raw,
        }
    }

    /// Lower-cased active-content string used for search and for "captured
    /// content" computations.
    pub fn search_text(&self) -> String {
        match self {
            Part::Text { content, .. } | Part::Reasoning { content, .. } => content.clone(),
            Part::ToolCall { name, id, arguments, .. } => {
                format!("{name} {id} {}", crate::tui::format::pretty_json(arguments))
            }
            Part::ToolCallResponse { id, result, .. } => {
                format!("{id} {}", match result {
                    Value::String(s) => s.clone(),
                    other => crate::tui::format::pretty_json(other),
                })
            }
            Part::Other { raw } => crate::tui::format::pretty_json(raw),
        }
    }
}

/// `attrs(span)` — return `span.attributes` if it is a non-null object;
/// otherwise parse `span.attributes_json` as JSON and return it if it parses
/// to a plain object; otherwise return an empty JSON object.
pub fn attrs(span: &Value) -> Value {
    if let Some(obj) = span.get("attributes").and_then(Value::as_object) {
        return Value::Object(obj.clone());
    }
    if let Some(s) = span.get("attributes_json").and_then(Value::as_str) {
        if let Ok(parsed) = serde_json::from_str::<Value>(s) {
            if parsed.is_object() {
                return parsed;
            }
        }
    }
    Value::Object(Default::default())
}

/// `hasCapturedContent(a)` — true iff at least one of the three content keys
/// is non-null.
pub fn has_captured_content(a: &Value) -> bool {
    for k in [
        "gen_ai.input.messages",
        "gen_ai.output.messages",
        "gen_ai.system_instructions",
    ] {
        match a.get(k) {
            Some(Value::Null) | None => continue,
            Some(_) => return true,
        }
    }
    false
}

/// Accept either an inline array OR a JSON-stringified array (per OTel
/// semconv) and return the parsed array. Returns `None` if neither form is
/// present or parsing fails.
fn parse_array_attr(attrs: &Value, key: &str) -> Option<Vec<Value>> {
    let v = attrs.as_object()?.get(key)?;
    match v {
        Value::Array(a) => Some(a.clone()),
        Value::String(s) => {
            let parsed: Value = serde_json::from_str(s).ok()?;
            parsed.as_array().cloned()
        }
        _ => None,
    }
}

/// `parseInputMessages` — accept inline OR JSON-stringified array at
/// `gen_ai.input.messages`; one `Message` per entry; default `role` to
/// `"unknown"`; preserve `finish_reason` when string.
pub fn parse_input_messages(attrs: &Value) -> Vec<Message> {
    let Some(arr) = parse_array_attr(attrs, "gen_ai.input.messages") else {
        return Vec::new();
    };
    arr.into_iter().map(parse_message).collect()
}

/// `parseOutputMessages` — same as [`parse_input_messages`] but reads
/// `gen_ai.output.messages`.
pub fn parse_output_messages(attrs: &Value) -> Vec<Message> {
    let Some(arr) = parse_array_attr(attrs, "gen_ai.output.messages") else {
        return Vec::new();
    };
    arr.into_iter().map(parse_message).collect()
}

fn parse_message(v: Value) -> Message {
    let role = v
        .get("role")
        .and_then(Value::as_str)
        .unwrap_or("unknown")
        .to_string();
    let parts_raw = v
        .get("parts")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let parts = parts_raw.into_iter().map(parse_part).collect();
    let finish_reason = v
        .get("finish_reason")
        .and_then(Value::as_str)
        .map(|s| s.to_string());
    Message {
        role,
        parts,
        finish_reason,
        raw: v,
    }
}

/// Dispatch on the `type` discriminator. Unknown values fall through to
/// `Other { raw }` per `Message view renders parts by type`.
pub fn parse_part(value: Value) -> Part {
    let ty = value.get("type").and_then(Value::as_str).unwrap_or("");
    match ty {
        "text" => {
            let content = value
                .get("content")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            Part::Text { content, raw: value }
        }
        "reasoning" => {
            let content = value
                .get("content")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            Part::Reasoning { content, raw: value }
        }
        "tool_call" => {
            let name = value
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            let id = value
                .get("id")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            let arguments = value
                .get("arguments")
                .cloned()
                .unwrap_or(Value::Null);
            Part::ToolCall { name, id, arguments, raw: value }
        }
        "tool_call_response" => {
            let id = value
                .get("id")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            let result = value
                .get("result")
                .cloned()
                .unwrap_or(Value::Null);
            Part::ToolCallResponse { id, result, raw: value }
        }
        _ => Part::Other { raw: value },
    }
}

/// `parseToolCallResult(a)` — three-way string/JSON/raw branching per the
/// LLR. Returns `None` when the key is null/absent.
pub fn parse_tool_call_result(attrs: &Value) -> Option<Value> {
    let v = attrs.as_object()?.get("gen_ai.tool.call.result")?;
    match v {
        Value::Null => None,
        Value::String(s) => match serde_json::from_str::<Value>(s) {
            Ok(Value::String(_)) => Some(Value::String(s.clone())),
            Ok(parsed) => Some(parsed),
            Err(_) => Some(Value::String(s.clone())),
        },
        other => Some(other.clone()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn attrs_returns_object_when_present() {
        let span = json!({"attributes": {"a": 1}});
        assert_eq!(attrs(&span), json!({"a": 1}));
    }

    #[test]
    fn attrs_falls_back_to_attributes_json() {
        let span = json!({"attributes": null, "attributes_json": "{\"b\":2}"});
        assert_eq!(attrs(&span), json!({"b": 2}));
    }

    #[test]
    fn attrs_returns_empty_on_bad_shape() {
        let span = json!({"attributes_json": "not json"});
        assert_eq!(attrs(&span), json!({}));
        let span = json!({"attributes_json": "[1,2,3]"}); // array, not object
        assert_eq!(attrs(&span), json!({}));
    }

    #[test]
    fn has_captured_content_when_any_key_set() {
        let a = json!({"gen_ai.input.messages": []});
        assert!(has_captured_content(&a));
        let a = json!({"gen_ai.input.messages": null, "gen_ai.output.messages": null});
        assert!(!has_captured_content(&a));
        let a = json!({});
        assert!(!has_captured_content(&a));
    }

    #[test]
    fn parse_input_messages_accepts_inline_array() {
        let a = json!({"gen_ai.input.messages": [
            {"role": "user", "parts": [{"type": "text", "content": "hi"}]}
        ]});
        let m = parse_input_messages(&a);
        assert_eq!(m.len(), 1);
        assert_eq!(m[0].role, "user");
        assert!(matches!(&m[0].parts[0], Part::Text { content, .. } if content == "hi"));
    }

    #[test]
    fn parse_input_messages_accepts_stringified_array() {
        let a = json!({"gen_ai.input.messages":
            "[{\"role\":\"assistant\",\"parts\":[]}]"
        });
        let m = parse_input_messages(&a);
        assert_eq!(m.len(), 1);
        assert_eq!(m[0].role, "assistant");
    }

    #[test]
    fn missing_role_defaults_to_unknown() {
        let a = json!({"gen_ai.input.messages": [{"parts": []}]});
        let m = parse_input_messages(&a);
        assert_eq!(m[0].role, "unknown");
    }

    #[test]
    fn finish_reason_preserved_when_string() {
        let a = json!({"gen_ai.output.messages": [
            {"role": "assistant", "parts": [], "finish_reason": "stop"}
        ]});
        let m = parse_output_messages(&a);
        assert_eq!(m[0].finish_reason.as_deref(), Some("stop"));

        let a = json!({"gen_ai.output.messages": [
            {"role": "assistant", "parts": [], "finish_reason": 42}
        ]});
        let m = parse_output_messages(&a);
        assert_eq!(m[0].finish_reason, None);
    }

    #[test]
    fn parse_part_dispatches_all_kinds() {
        assert!(matches!(
            parse_part(json!({"type": "text", "content": "x"})),
            Part::Text { .. }
        ));
        assert!(matches!(
            parse_part(json!({"type": "reasoning", "content": "y"})),
            Part::Reasoning { .. }
        ));
        assert!(matches!(
            parse_part(json!({"type": "tool_call", "name": "ls", "id": "x1", "arguments": {}})),
            Part::ToolCall { .. }
        ));
        assert!(matches!(
            parse_part(json!({"type": "tool_call_response", "id": "x1", "result": "ok"})),
            Part::ToolCallResponse { .. }
        ));
        assert!(matches!(
            parse_part(json!({"type": "future_type", "data": 1})),
            Part::Other { .. }
        ));
        // No `type` field at all → Other.
        assert!(matches!(parse_part(json!({})), Part::Other { .. }));
    }

    #[test]
    fn parse_tool_call_result_three_way() {
        // null/absent → None.
        assert_eq!(parse_tool_call_result(&json!({})), None);
        assert_eq!(parse_tool_call_result(&json!({"gen_ai.tool.call.result": null})), None);

        // String containing JSON object → parsed object.
        let v = parse_tool_call_result(&json!({"gen_ai.tool.call.result": "{\"x\":1}"})).unwrap();
        assert_eq!(v, json!({"x": 1}));

        // String containing JSON string primitive → raw verbatim.
        let v = parse_tool_call_result(&json!({"gen_ai.tool.call.result": "\"hello\""})).unwrap();
        assert_eq!(v, json!("\"hello\""));

        // Malformed string → raw verbatim.
        let v = parse_tool_call_result(&json!({"gen_ai.tool.call.result": "not json"})).unwrap();
        assert_eq!(v, json!("not json"));

        // Inline object → cloned through.
        let v = parse_tool_call_result(&json!({"gen_ai.tool.call.result": {"a": 1}})).unwrap();
        assert_eq!(v, json!({"a": 1}));
    }
}
