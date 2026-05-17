//! Session export: dumps every `real` span (and its events) that belongs to a
//! given `gen_ai.conversation.id` as one JSON-lines envelope per row, in the
//! same shape the file-exporter emits and that `ghcp-mon replay` ingests.
//!
//! Span-only export — metrics and logs are intentionally excluded. Spans are
//! the canonical projection for the dashboard; metric/log replay can be added
//! later if needed.

use crate::error::{AppError, AppResult};
use crate::model::{HrTime, InstrumentationScope, Resource, SpanEnvelope, SpanStatus, EventEnvelope};
use serde_json::{Map, Value};
use sqlx::SqlitePool;
use tokio::io::AsyncWriteExt;
use tracing::warn;

/// Return Ok(true) when a `sessions` row exists for `conv_id`. Used as a
/// preflight before opening the output file so missing sessions don't
/// produce an empty/truncated file.
pub async fn session_exists(pool: &SqlitePool, conv_id: &str) -> AppResult<bool> {
    let row: Option<(i64,)> = sqlx::query_as(
        "SELECT 1 FROM sessions WHERE conversation_id = ? LIMIT 1"
    )
    .bind(conv_id)
    .fetch_optional(pool)
    .await?;
    Ok(row.is_some())
}

/// Stream every `real` span belonging to the session as JSON-lines into
/// `writer`. Membership: any span carrying the conv id directly, plus every
/// span sharing a `trace_id` with such a span, plus spans referenced by any
/// agent_run/chat_turn/tool_call/external_tool_call projection tagged with
/// the conv id (transitively via the same trace_id grouping). Returns
/// `NotFound` if the `sessions` row does not exist.
pub async fn export_session<W>(
    pool: &SqlitePool,
    conv_id: &str,
    writer: &mut W,
) -> AppResult<usize>
where
    W: AsyncWriteExt + Unpin,
{
    if !session_exists(pool, conv_id).await? {
        return Err(AppError::NotFound);
    }

    // Span query — `trace_id IN ()` rolls together direct-attribute matches
    // and projection-referenced matches, then expands to every sibling span
    // in those traces so parent/child chains stay intact on replay.
    let rows: Vec<SpanRow> = sqlx::query_as::<_, SpanRow>(
        r#"
        SELECT
            span_pk, trace_id, span_id, parent_span_id, name, kind,
            start_unix_ns, end_unix_ns, status_code, status_message,
            attributes_json, resource_json, scope_name, scope_version
        FROM spans
        WHERE ingestion_state = 'real'
          AND trace_id IN (
            SELECT trace_id FROM spans
             WHERE json_extract(attributes_json, '$."gen_ai.conversation.id"') = ?1
            UNION
            SELECT s2.trace_id FROM spans s2 WHERE s2.span_pk IN (
                SELECT span_pk FROM agent_runs        WHERE conversation_id = ?1
                UNION ALL
                SELECT span_pk FROM chat_turns        WHERE conversation_id = ?1
                UNION ALL
                SELECT span_pk FROM tool_calls        WHERE conversation_id = ?1
                UNION ALL
                SELECT span_pk FROM external_tool_calls WHERE conversation_id = ?1
            )
          )
        ORDER BY (start_unix_ns IS NULL), start_unix_ns ASC, span_pk ASC
        "#,
    )
    .bind(conv_id)
    .fetch_all(pool)
    .await?;

    let mut count = 0usize;
    for row in &rows {
        let events = fetch_events_for(pool, row.span_pk).await?;
        let env = row.to_envelope(events)?;
        // Bare SpanEnvelope (not Envelope::Span) — replay parses via the
        // `Envelope` enum's internal `type` tag, which dispatches on the
        // `type:"span"` field already present in SpanEnvelope.
        let line = serde_json::to_string(&env)?;
        writer.write_all(line.as_bytes()).await?;
        writer.write_all(b"\n").await?;
        count += 1;
    }
    writer.flush().await?;
    Ok(count)
}

async fn fetch_events_for(pool: &SqlitePool, span_pk: i64) -> AppResult<Vec<EventEnvelope>> {
    let rows: Vec<(String, i64, String)> = sqlx::query_as(
        "SELECT name, time_unix_ns, attributes_json FROM span_events \
         WHERE span_pk = ? ORDER BY time_unix_ns ASC, event_pk ASC",
    )
    .bind(span_pk)
    .fetch_all(pool)
    .await?;
    let mut out = Vec::with_capacity(rows.len());
    for (name, ns, attrs_json) in rows {
        let attributes = parse_attr_map(&attrs_json);
        out.push(EventEnvelope {
            name,
            time: HrTime::Nanos(ns),
            attributes,
        });
    }
    Ok(out)
}

#[derive(sqlx::FromRow)]
struct SpanRow {
    span_pk: i64,
    trace_id: String,
    span_id: String,
    parent_span_id: Option<String>,
    name: String,
    kind: Option<i64>,
    start_unix_ns: Option<i64>,
    end_unix_ns: Option<i64>,
    status_code: Option<i64>,
    status_message: Option<String>,
    attributes_json: String,
    resource_json: Option<String>,
    scope_name: Option<String>,
    scope_version: Option<String>,
}

impl SpanRow {
    fn to_envelope(&self, events: Vec<EventEnvelope>) -> AppResult<SpanEnvelope> {
        let attributes = parse_attr_map(&self.attributes_json);
        let resource = self.resource_json.as_deref().and_then(|s| {
            match serde_json::from_str::<Resource>(s) {
                Ok(r) => Some(r),
                Err(e) => {
                    warn!(span_pk = self.span_pk, "resource_json decode failed: {e}");
                    None
                }
            }
        });
        let instrumentation_scope = match (&self.scope_name, &self.scope_version) {
            (None, None) => None,
            (n, v) => Some(InstrumentationScope { name: n.clone(), version: v.clone() }),
        };
        let status = self.status_code.map(|code| SpanStatus {
            code,
            message: self.status_message.clone(),
        });
        if self.start_unix_ns.is_none() {
            // Schema permits NULL on real spans; defensive fallback to 0 so
            // the replayed row still inserts.
            warn!(span_pk = self.span_pk, span_id = %self.span_id, "exporting span with NULL start_unix_ns");
        }
        Ok(SpanEnvelope {
            kind_tag: "span".into(),
            trace_id: self.trace_id.clone(),
            span_id: self.span_id.clone(),
            parent_span_id: self.parent_span_id.clone(),
            name: self.name.clone(),
            kind: self.kind,
            start_time: HrTime::Nanos(self.start_unix_ns.unwrap_or(0)),
            end_time: self.end_unix_ns.map(HrTime::Nanos),
            attributes,
            events,
            status,
            resource,
            instrumentation_scope,
        })
    }
}

fn parse_attr_map(s: &str) -> Map<String, Value> {
    match serde_json::from_str::<Value>(s) {
        Ok(Value::Object(m)) => m,
        _ => Map::new(),
    }
}
