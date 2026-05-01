---
type: cheatsheet
---
Source: `src/model.rs`. Crate path: `ghcp_mon::model`.

## Extract

```rust
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum HrTime { Pair([i64; 2]), Nanos(i64) }

impl HrTime { pub fn to_unix_nanos(&self) -> i64; }

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Resource {
    #[serde(default)] pub attributes: Map<String, Value>,
    #[serde(rename = "schemaUrl", default, skip_serializing_if = "Option::is_none")]
    pub schema_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct InstrumentationScope {
    pub name: Option<String>,
    pub version: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SpanStatus { pub code: i64, pub message: Option<String> }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventEnvelope {
    pub name: String,
    pub time: HrTime,
    #[serde(default)] pub attributes: Map<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpanEnvelope {
    #[serde(rename = "type", default = "default_span_type")] pub kind_tag: String, // "span"
    #[serde(rename = "traceId")] pub trace_id: String,
    #[serde(rename = "spanId")]  pub span_id: String,
    #[serde(rename = "parentSpanId", default, skip_serializing_if = "Option::is_none")]
    pub parent_span_id: Option<String>,
    pub name: String,
    #[serde(default)] pub kind: Option<i64>,
    #[serde(rename = "startTime")] pub start_time: HrTime,
    #[serde(rename = "endTime", default)] pub end_time: Option<HrTime>,
    #[serde(default)] pub attributes: Map<String, Value>,
    #[serde(default)] pub events: Vec<EventEnvelope>,
    #[serde(default)] pub status: Option<SpanStatus>,
    #[serde(default)] pub resource: Option<Resource>,
    #[serde(rename = "instrumentationScope", default)]
    pub instrumentation_scope: Option<InstrumentationScope>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricDataPoint {
    #[serde(default)] pub attributes: Map<String, Value>,
    #[serde(rename = "startTime", default)] pub start_time: Option<HrTime>,
    #[serde(rename = "endTime",   default)] pub end_time:   Option<HrTime>,
    pub value: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricEnvelope {
    #[serde(rename = "type", default = "default_metric_type")] pub kind_tag: String,
    pub name: String,
    pub description: Option<String>,
    pub unit: Option<String>,
    #[serde(rename = "dataPoints", default)] pub data_points: Vec<MetricDataPoint>,
    pub resource: Option<Resource>,
    #[serde(rename = "instrumentationScope", default)]
    pub instrumentation_scope: Option<InstrumentationScope>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEnvelope {
    #[serde(rename = "type", default = "default_log_type")] pub kind_tag: String,
    #[serde(default)] pub body: Value,
    #[serde(default)] pub attributes: Map<String, Value>,
    #[serde(rename = "timeUnixNano", default)] pub time_unix_nano: Option<HrTime>,
    pub resource: Option<Resource>,
    #[serde(rename = "instrumentationScope", default)]
    pub instrumentation_scope: Option<InstrumentationScope>,
    pub severity_text: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Envelope {
    Span(Box<SpanEnvelope>),
    Metric(Box<MetricEnvelope>),
    Log(Box<LogEnvelope>),
}

impl Envelope { pub fn type_tag(&self) -> &'static str; }

pub fn attr_str<'a>(m: &'a Map<String, Value>, key: &str) -> Option<&'a str>;
pub fn attr_i64(m: &Map<String, Value>, key: &str) -> Option<i64>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpanKindClass { InvokeAgent, Chat, ExecuteTool, ExternalTool, Other }

impl SpanKindClass {
    pub fn from_name(name: &str) -> Self;
    pub fn as_str(self) -> &'static str;
}
```

## Suggested Test Strategy

- Pure data + classifier — no async, no I/O. Use plain `#[test]`.
- `Envelope` is **internally tagged** (`#[serde(tag = "type", rename_all = "lowercase")]`), so test JSON like `{"type":"span", ...}` / `{"type":"metric", ...}` / `{"type":"log", ...}` and confirm `serde_json::from_str::<Envelope>(...)` produces the matching variant. Round-trip with `serde_json::to_string` and re-parse.
- `SpanKindClass::from_name` mapping rules (covered behavior contract):
  - `"invoke_agent"` exact OR `"invoke_agent "` prefix → `InvokeAgent`
  - `"chat"`-prefix → `Chat`
  - `"execute_tool"`-prefix → `ExecuteTool`
  - `"external_tool"`-prefix → `ExternalTool`
  - everything else → `Other`
- `as_str` round-trip: `from_name(c.as_str())` returns the same variant for all five `as_str` outputs (`"invoke_agent"`, `"chat"`, `"execute_tool"`, `"external_tool"`, `"other"`). Use `assert_eq!` with `PartialEq` derive.
- For `HrTime`: cover the untagged enum — array `[s, n]` and integer `n` both deserialize. `to_unix_nanos` uses saturating arithmetic on the pair branch.
- For per-field renames: build a concrete `SpanEnvelope` programmatically and serialize, then assert the resulting JSON has keys `traceId`, `spanId`, `parentSpanId`, `startTime`, `endTime`, `instrumentationScope`. `parent_span_id: None` is skipped (`skip_serializing_if = "Option::is_none"`).
