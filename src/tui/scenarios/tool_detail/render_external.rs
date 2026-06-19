//! External tool-detail body — for spans backed only by an
//! `external_tool_call` projection (MCP / external-origin tools). Renders the
//! external-specific metadata header, then the generic args/result renderer and
//! the collapsible raw-attributes JSON. Mirrors the web `ExternalToolDetailBody`.
//!
//! Source for (shared `frontend/llr/`):
//! - `External tool detail body header fields`
//! - `External tool detail uses generic args renderer`

use serde_json::Value;

use super::{render_generic, BodyCtx};
use crate::tui::format::{fmt_clock, fmt_ns};
use crate::tui::model::SpanDetail;

/// Render the full external tool-detail body.
pub(crate) fn render(ctx: &mut BodyCtx, detail: &SpanDetail) {
    let ext = detail
        .projection
        .external_tool_call
        .as_ref()
        .expect("external route implies external_tool_call");
    let span = &detail.span;
    let dur = span.duration_ns.map(|d| d as i128).or_else(|| {
        match (span.start_unix_ns, span.end_unix_ns) {
            (Some(s), Some(e)) => Some(e - s),
            _ => None,
        }
    });
    let tool_name = ext.tool_name.clone().unwrap_or_else(|| "(unknown tool)".to_string());
    let kv = vec![
        ("call_id".to_string(), ext.call_id.clone().unwrap_or_else(|| "—".to_string())),
        ("tool_type".to_string(), "external".to_string()),
        ("duration".to_string(), fmt_ns(dur)),
        ("start".to_string(), fmt_clock(span.start_unix_ns)),
        (
            "conv".to_string(),
            ext.conversation_id
                .as_ref()
                .map(|c| c.chars().take(8).collect::<String>())
                .unwrap_or_else(|| "—".to_string()),
        ),
        (
            "paired_tool_call_pk".to_string(),
            ext.paired_tool_call_pk.map(|v| v.to_string()).unwrap_or_else(|| "—".to_string()),
        ),
        (
            "agent_run_pk".to_string(),
            ext.agent_run_pk.map(|v| v.to_string()).unwrap_or_else(|| "—".to_string()),
        ),
    ];
    ctx.metadata_panel(&tool_name, &kv);

    ctx.gap();
    ctx.label("args / result");
    let attrs = span.attributes.clone().unwrap_or(Value::Null);
    render_generic::render(ctx, &attrs);

    ctx.gap();
    ctx.label("raw span attributes");
    let raw = span.attributes.clone().unwrap_or(Value::Null);
    ctx.json_panel("raw_attrs", &raw);
}
