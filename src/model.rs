//! Internal envelope shapes shared by ingest, normalize, and storage paths.
//!
//! Anchored on the file-exporter JSON-lines format. OTLP/HTTP requests are
//! converted into these envelopes before normalization.

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// `[seconds, nanoseconds]` pair as emitted by the file exporter.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum HrTime {
    Pair([i64; 2]),
    Nanos(i64),
}

impl HrTime {
    pub fn to_unix_nanos(&self) -> i64 {
        match self {
            HrTime::Pair([s, n]) => s.saturating_mul(1_000_000_000).saturating_add(*n),
            HrTime::Nanos(n) => *n,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Resource {
    #[serde(default)]
    pub attributes: Map<String, Value>,
    #[serde(rename = "schemaUrl", default, skip_serializing_if = "Option::is_none")]
    pub schema_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct InstrumentationScope {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SpanStatus {
    #[serde(default)]
    pub code: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventEnvelope {
    pub name: String,
    pub time: HrTime,
    #[serde(default)]
    pub attributes: Map<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpanEnvelope {
    #[serde(rename = "type", default = "default_span_type")]
    pub kind_tag: String, // "span"
    #[serde(rename = "traceId")]
    pub trace_id: String,
    #[serde(rename = "spanId")]
    pub span_id: String,
    #[serde(rename = "parentSpanId", default, skip_serializing_if = "Option::is_none")]
    pub parent_span_id: Option<String>,
    pub name: String,
    #[serde(default)]
    pub kind: Option<i64>,
    #[serde(rename = "startTime")]
    pub start_time: HrTime,
    #[serde(rename = "endTime", default)]
    pub end_time: Option<HrTime>,
    #[serde(default)]
    pub attributes: Map<String, Value>,
    #[serde(default)]
    pub events: Vec<EventEnvelope>,
    #[serde(default)]
    pub status: Option<SpanStatus>,
    #[serde(default)]
    pub resource: Option<Resource>,
    #[serde(rename = "instrumentationScope", default)]
    pub instrumentation_scope: Option<InstrumentationScope>,
}

fn default_span_type() -> String { "span".into() }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricDataPoint {
    #[serde(default)]
    pub attributes: Map<String, Value>,
    #[serde(rename = "startTime", default)]
    pub start_time: Option<HrTime>,
    #[serde(rename = "endTime", default)]
    pub end_time: Option<HrTime>,
    pub value: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricEnvelope {
    #[serde(rename = "type", default = "default_metric_type")]
    pub kind_tag: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unit: Option<String>,
    #[serde(rename = "dataPoints", default)]
    pub data_points: Vec<MetricDataPoint>,
    #[serde(default)]
    pub resource: Option<Resource>,
    #[serde(rename = "instrumentationScope", default)]
    pub instrumentation_scope: Option<InstrumentationScope>,
}

fn default_metric_type() -> String { "metric".into() }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEnvelope {
    #[serde(rename = "type", default = "default_log_type")]
    pub kind_tag: String,
    #[serde(default)]
    pub body: Value,
    #[serde(default)]
    pub attributes: Map<String, Value>,
    #[serde(rename = "timeUnixNano", default)]
    pub time_unix_nano: Option<HrTime>,
    #[serde(default)]
    pub resource: Option<Resource>,
    #[serde(rename = "instrumentationScope", default)]
    pub instrumentation_scope: Option<InstrumentationScope>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub severity_text: Option<String>,
}

fn default_log_type() -> String { "log".into() }

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Envelope {
    Span(Box<SpanEnvelope>),
    Metric(Box<MetricEnvelope>),
    Log(Box<LogEnvelope>),
}

impl Envelope {
    pub fn type_tag(&self) -> &'static str {
        match self {
            Envelope::Span(_) => "span",
            Envelope::Metric(_) => "metric",
            Envelope::Log(_) => "log",
        }
    }
}

/// Helper: read a string attribute.
pub fn attr_str<'a>(m: &'a Map<String, Value>, key: &str) -> Option<&'a str> {
    m.get(key).and_then(|v| v.as_str())
}

pub fn attr_i64(m: &Map<String, Value>, key: &str) -> Option<i64> {
    m.get(key).and_then(|v| v.as_i64().or_else(|| v.as_f64().map(|f| f as i64)))
}

/// Classifier shared by ingest/normalize/api — derived purely from `name`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpanKindClass {
    InvokeAgent,
    Chat,
    ExecuteTool,
    ExternalTool,
    Other,
}

impl SpanKindClass {
    pub fn from_name(name: &str) -> Self {
        if name == "invoke_agent" || name.starts_with("invoke_agent ") {
            SpanKindClass::InvokeAgent
        } else if name.starts_with("chat") {
            SpanKindClass::Chat
        } else if name.starts_with("execute_tool") {
            SpanKindClass::ExecuteTool
        } else if name.starts_with("external_tool") {
            SpanKindClass::ExternalTool
        } else {
            SpanKindClass::Other
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            SpanKindClass::InvokeAgent => "invoke_agent",
            SpanKindClass::Chat => "chat",
            SpanKindClass::ExecuteTool => "execute_tool",
            SpanKindClass::ExternalTool => "external_tool",
            SpanKindClass::Other => "other",
        }
    }
}
