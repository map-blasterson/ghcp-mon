//! Rust ports of `web/src/api/types.ts` — serde-deserializable mirrors of
//! the wire model exposed by `src/api/*`. Open-ended payloads
//! (`WsEnvelope::payload`, attribute maps) use [`serde_json::Value`].

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub type UnixNs = i128;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KindClass {
    InvokeAgent,
    Chat,
    ExecuteTool,
    ExternalTool,
    Other,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SessionSummary {
    pub conversation_id: String,
    pub first_seen_ns: Option<UnixNs>,
    pub last_seen_ns: Option<UnixNs>,
    pub latest_model: Option<String>,
    pub chat_turn_count: i64,
    pub tool_call_count: i64,
    pub agent_run_count: i64,
    pub service_name: Option<String>,
    #[serde(default)]
    pub local_name: Option<String>,
    #[serde(default)]
    pub user_named: Option<bool>,
    #[serde(default)]
    pub cwd: Option<String>,
    #[serde(default)]
    pub branch: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ListSessionsResponse {
    pub sessions: Vec<SessionSummary>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SessionDetail {
    #[serde(flatten)]
    pub summary: SessionSummary,
    pub span_count: i64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SpanRow {
    pub span_pk: i64,
    pub trace_id: String,
    pub span_id: String,
    pub parent_span_id: Option<String>,
    pub name: String,
    pub kind_class: KindClass,
    pub start_unix_ns: Option<UnixNs>,
    pub end_unix_ns: Option<UnixNs>,
    pub ingestion_state: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ListSpansResponse {
    pub spans: Vec<SpanRow>,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct ChatTurnProjection {
    pub turn_pk: i64,
    pub conversation_id: Option<String>,
    pub agent_run_pk: Option<i64>,
    pub interaction_id: Option<String>,
    pub turn_id: Option<String>,
    pub model: Option<String>,
    pub input_tokens: Option<i64>,
    pub output_tokens: Option<i64>,
    pub cache_read_tokens: Option<i64>,
    pub reasoning_tokens: Option<i64>,
    pub tool_call_count: i64,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct ToolCallProjection {
    pub tool_call_pk: i64,
    pub call_id: Option<String>,
    pub tool_name: Option<String>,
    pub tool_type: Option<String>,
    pub conversation_id: Option<String>,
    pub agent_run_pk: Option<i64>,
    pub status_code: Option<i64>,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct AgentRunProjection {
    pub agent_run_pk: i64,
    pub conversation_id: Option<String>,
    pub agent_id: Option<String>,
    pub agent_name: Option<String>,
    pub agent_version: Option<String>,
    pub parent_agent_run_pk: Option<i64>,
    pub parent_span_pk: Option<i64>,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct ExternalToolCallProjection {
    pub ext_pk: i64,
    pub call_id: Option<String>,
    pub tool_name: Option<String>,
    pub paired_tool_call_pk: Option<i64>,
    pub conversation_id: Option<String>,
    pub agent_run_pk: Option<i64>,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct SpanProjection {
    #[serde(default)]
    pub chat_turn: Option<ChatTurnProjection>,
    #[serde(default)]
    pub tool_call: Option<ToolCallProjection>,
    #[serde(default)]
    pub agent_run: Option<AgentRunProjection>,
    #[serde(default)]
    pub external_tool_call: Option<ExternalToolCallProjection>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SpanRef {
    pub span_pk: i64,
    pub trace_id: String,
    pub span_id: String,
    pub name: String,
    pub kind_class: KindClass,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SpanEvent {
    pub event_pk: i64,
    pub name: String,
    pub time_unix_ns: UnixNs,
    pub attributes: Option<Value>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SpanFull {
    pub span_pk: i64,
    pub trace_id: String,
    pub span_id: String,
    pub parent_span_id: Option<String>,
    pub name: String,
    // OTel `SpanKind` enum encoded as a small integer (0..=5) by the
    // collector. The backend's `gen_ai` projection uses the string-valued
    // `kind_class` instead, so we never read this — but the wire format
    // is integer-or-null. Typing it as `String` (as the webui does in
    // its untyped JS world) made every `SpanDetail` fetch fail to
    // deserialize, silently breaking chips and the span detail pane.
    pub kind: Option<i64>,
    pub kind_class: KindClass,
    pub start_unix_ns: Option<UnixNs>,
    pub end_unix_ns: Option<UnixNs>,
    pub duration_ns: Option<i64>,
    pub status_message: Option<String>,
    pub ingestion_state: String,
    pub scope_name: Option<String>,
    pub scope_version: Option<String>,
    pub attributes: Option<Value>,
    pub resource: Option<Value>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SpanDetail {
    pub span: SpanFull,
    pub events: Vec<SpanEvent>,
    pub parent: Option<SpanRef>,
    pub children: Vec<SpanRef>,
    pub projection: SpanProjection,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SpanNode {
    pub span_pk: i64,
    pub trace_id: String,
    pub span_id: String,
    pub parent_span_id: Option<String>,
    pub name: String,
    pub kind_class: KindClass,
    pub ingestion_state: String,
    pub start_unix_ns: Option<UnixNs>,
    pub end_unix_ns: Option<UnixNs>,
    pub projection: SpanProjection,
    pub children: Vec<SpanNode>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SessionSpanTreeResponse {
    pub conversation_id: String,
    pub tree: Vec<SpanNode>,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct KindCounts {
    #[serde(default)]
    pub chat: i64,
    #[serde(default)]
    pub execute_tool: i64,
    #[serde(default)]
    pub external_tool: i64,
    #[serde(default)]
    pub invoke_agent: i64,
    #[serde(default)]
    pub other: i64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TraceRootRef {
    pub span_pk: i64,
    pub trace_id: String,
    pub span_id: String,
    pub parent_span_id: Option<String>,
    pub name: String,
    pub kind_class: KindClass,
    pub ingestion_state: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TraceSummary {
    pub trace_id: String,
    pub first_seen_ns: Option<UnixNs>,
    pub last_seen_ns: Option<UnixNs>,
    pub span_count: i64,
    pub placeholder_count: i64,
    pub kind_counts: KindCounts,
    pub root: Option<TraceRootRef>,
    pub conversation_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ListTracesResponse {
    pub traces: Vec<TraceSummary>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TraceDetailResponse {
    pub trace_id: String,
    pub conversation_id: Option<String>,
    pub tree: Vec<SpanNode>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ContextSnapshot {
    pub ctx_pk: i64,
    pub span_pk: Option<i64>,
    pub captured_ns: UnixNs,
    pub token_limit: Option<i64>,
    pub current_tokens: Option<i64>,
    pub messages_length: Option<i64>,
    pub input_tokens: Option<i64>,
    pub output_tokens: Option<i64>,
    pub cache_read_tokens: Option<i64>,
    pub reasoning_tokens: Option<i64>,
    pub source: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ListSessionContextsResponse {
    pub conversation_id: String,
    pub context_snapshots: Vec<ContextSnapshot>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RawRecordType {
    Span,
    Metric,
    Log,
    #[serde(rename = "otlp-traces")]
    OtlpTraces,
    #[serde(rename = "otlp-metrics")]
    OtlpMetrics,
    #[serde(rename = "otlp-logs")]
    OtlpLogs,
    #[serde(rename = "envelope-batch")]
    EnvelopeBatch,
}

impl RawRecordType {
    /// Wire-form string of the variant; mirrors `RawRecordType` in webui.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Span => "span",
            Self::Metric => "metric",
            Self::Log => "log",
            Self::OtlpTraces => "otlp-traces",
            Self::OtlpMetrics => "otlp-metrics",
            Self::OtlpLogs => "otlp-logs",
            Self::EnvelopeBatch => "envelope-batch",
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RawRecord {
    pub id: i64,
    pub received_at: String,
    pub source: String,
    pub record_type: RawRecordType,
    pub content_type: Option<String>,
    pub body: Value,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ListRawResponse {
    pub raw: Vec<RawRecord>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SearchMatch {
    pub field: String,
    pub fragment: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SearchSpanResult {
    pub span_pk: i64,
    pub trace_id: String,
    pub span_id: String,
    pub parent_span_id: Option<String>,
    pub name: String,
    pub kind_class: KindClass,
    pub start_unix_ns: Option<UnixNs>,
    pub end_unix_ns: Option<UnixNs>,
    pub ingestion_state: String,
    pub projection: SpanProjection,
    pub matches: Vec<SearchMatch>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SearchResponse {
    pub results: Vec<SearchSpanResult>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WsKind {
    Hello,
    Span,
    Metric,
    Log,
    Derived,
    Trace,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WsEntity {
    Control,
    Span,
    Placeholder,
    Metric,
    Log,
    Session,
    ChatTurn,
    ToolCall,
    ExternalToolCall,
    AgentRun,
    ContextSnapshot,
    Trace,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WsEnvelope {
    pub kind: WsKind,
    pub entity: WsEntity,
    pub payload: Value,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DeleteSessionResponse {
    pub deleted: bool,
    pub conversation_id: String,
    pub trace_count: i64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kind_class_round_trip() {
        let kc = KindClass::ExecuteTool;
        let s = serde_json::to_string(&kc).unwrap();
        assert_eq!(s, "\"execute_tool\"");
        let back: KindClass = serde_json::from_str(&s).unwrap();
        assert_eq!(back, kc);
    }

    #[test]
    fn ws_envelope_parses() {
        let raw = r#"{"kind":"derived","entity":"chat_turn","payload":{"action":"upsert"}}"#;
        let env: WsEnvelope = serde_json::from_str(raw).unwrap();
        assert_eq!(env.kind, WsKind::Derived);
        assert_eq!(env.entity, WsEntity::ChatTurn);
    }

    #[test]
    fn raw_record_type_wire_form() {
        let v = RawRecordType::OtlpTraces;
        let s = serde_json::to_string(&v).unwrap();
        assert_eq!(s, "\"otlp-traces\"");
        assert_eq!(v.as_str(), "otlp-traces");
    }

    /// Regression: the backend's `/api/spans/:trace/:span` response emits
    /// `kind` as an OTel `SpanKind` integer (or null), NOT a string. A real
    /// captured payload from `serve` is reproduced here so this can never
    /// silently regress again — a typed-parse failure here makes every
    /// downstream consumer of `SpanDetail` (chips, span detail pane, file
    /// touches, tool detail, etc.) render as if the fetch never happened.
    #[test]
    fn span_detail_parses_integer_kind_from_wire_payload() {
        let raw = r#"{
            "span": {
                "span_pk": 31000,
                "trace_id": "324d6bbc1d81377d1e5f0e63e01e4fc7",
                "span_id": "b84aabf9f8269309",
                "parent_span_id": "7d713b4e913ee358",
                "name": "execute_tool view",
                "kind": 1,
                "kind_class": "execute_tool",
                "start_unix_ns": 1780345541165000000,
                "end_unix_ns": 1780345541180069986,
                "duration_ns": 15069986,
                "status_message": null,
                "ingestion_state": "real",
                "scope_name": "github.copilot",
                "scope_version": "1.0.57-5",
                "attributes": {"gen_ai.tool.name": "view"},
                "resource": null
            },
            "events": [],
            "parent": null,
            "children": [],
            "projection": {}
        }"#;
        let d: SpanDetail = serde_json::from_str(raw).expect("must parse integer kind");
        assert_eq!(d.span.kind, Some(1));
        assert_eq!(d.span.kind_class, KindClass::ExecuteTool);
    }

    #[test]
    fn span_detail_parses_null_kind() {
        let raw = r#"{
            "span": {
                "span_pk": 1, "trace_id": "t", "span_id": "s",
                "parent_span_id": null, "name": "n", "kind": null,
                "kind_class": "other", "start_unix_ns": null,
                "end_unix_ns": null, "duration_ns": null,
                "status_message": null, "ingestion_state": "real",
                "scope_name": null, "scope_version": null,
                "attributes": null, "resource": null
            },
            "events": [], "parent": null, "children": [], "projection": {}
        }"#;
        let d: SpanDetail = serde_json::from_str(raw).expect("must parse null kind");
        assert_eq!(d.span.kind, None);
    }
}
