//! Normalize: spans-as-canonical-truth ingest with idempotent reconcile.

use crate::model::*;
use crate::ws::{Broadcaster, EventMsg};
use serde_json::json;
use sqlx::SqlitePool;
use tracing::debug;

pub struct NormalizeCtx<'a> {
    pub pool: &'a SqlitePool,
    pub bus: &'a Broadcaster,
    pub raw_record_id: i64,
}

pub async fn handle_envelope(ctx: &NormalizeCtx<'_>, env: &Envelope) -> anyhow::Result<()> {
    match env {
        Envelope::Span(s) => normalize_span(ctx, s).await?,
        Envelope::Metric(m) => normalize_metric(ctx, m).await?,
        Envelope::Log(_) => {}
    }
    Ok(())
}

async fn normalize_span(ctx: &NormalizeCtx<'_>, s: &SpanEnvelope) -> anyhow::Result<()> {
    let start_ns = s.start_time.to_unix_nanos();
    let end_ns = s.end_time.as_ref().map(|t| t.to_unix_nanos());
    let duration_ns = end_ns.map(|e| e - start_ns);

    let attrs_json = serde_json::to_string(&s.attributes)?;
    let resource_json = s.resource.as_ref().map(|r| serde_json::to_string(r)).transpose()?;
    let (scope_name, scope_version) = match &s.instrumentation_scope {
        Some(sc) => (sc.name.clone(), sc.version.clone()),
        None => (None, None),
    };
    let status_code = s.status.as_ref().map(|st| st.code);
    let status_msg = s.status.as_ref().and_then(|st| st.message.clone());

    let prev: Option<(i64, String)> = sqlx::query_as(
        "SELECT span_pk, ingestion_state FROM spans WHERE trace_id = ? AND span_id = ?"
    ).bind(&s.trace_id).bind(&s.span_id).fetch_optional(ctx.pool).await?;
    let was_placeholder = matches!(prev, Some((_, ref st)) if st == "placeholder");

    let span_pk: i64 = sqlx::query_scalar(
        "INSERT INTO spans(trace_id, span_id, parent_span_id, name, kind, \
         start_unix_ns, end_unix_ns, duration_ns, status_code, status_message, \
         attributes_json, resource_json, scope_name, scope_version, \
         ingestion_state, first_seen_raw_id, last_seen_raw_id) \
         VALUES(?,?,?,?,?,?,?,?,?,?,?,?,?,?,'real',?,?) \
         ON CONFLICT(trace_id, span_id) DO UPDATE SET \
            parent_span_id = excluded.parent_span_id, \
            name = excluded.name, \
            kind = excluded.kind, \
            start_unix_ns = excluded.start_unix_ns, \
            end_unix_ns = excluded.end_unix_ns, \
            duration_ns = excluded.duration_ns, \
            status_code = excluded.status_code, \
            status_message = excluded.status_message, \
            attributes_json = excluded.attributes_json, \
            resource_json = COALESCE(excluded.resource_json, spans.resource_json), \
            scope_name = COALESCE(excluded.scope_name, spans.scope_name), \
            scope_version = COALESCE(excluded.scope_version, spans.scope_version), \
            ingestion_state = 'real', \
            last_seen_raw_id = excluded.last_seen_raw_id \
         RETURNING span_pk"
    )
    .bind(&s.trace_id).bind(&s.span_id).bind(&s.parent_span_id)
    .bind(&s.name).bind(s.kind)
    .bind(start_ns).bind(end_ns).bind(duration_ns)
    .bind(status_code).bind(&status_msg)
    .bind(&attrs_json).bind(&resource_json).bind(&scope_name).bind(&scope_version)
    .bind(ctx.raw_record_id).bind(ctx.raw_record_id)
    .fetch_one(ctx.pool).await?;

    sqlx::query("DELETE FROM span_events WHERE span_pk = ?")
        .bind(span_pk).execute(ctx.pool).await?;

    for ev in &s.events {
        let ev_attrs = serde_json::to_string(&ev.attributes)?;
        sqlx::query(
            "INSERT INTO span_events(span_pk, raw_record_id, name, time_unix_ns, attributes_json) \
             VALUES(?,?,?,?,?)"
        )
        .bind(span_pk).bind(ctx.raw_record_id).bind(&ev.name)
        .bind(ev.time.to_unix_nanos()).bind(&ev_attrs)
        .execute(ctx.pool).await?;
    }

    if let Some(psid) = s.parent_span_id.as_deref().filter(|p| !p.is_empty()) {
        ensure_placeholder(ctx, &s.trace_id, psid).await?;
    }

    let kind_class = SpanKindClass::from_name(&s.name);
    match kind_class {
        SpanKindClass::InvokeAgent => upsert_agent_run(ctx, span_pk, s, start_ns, end_ns, duration_ns).await?,
        SpanKindClass::Chat => upsert_chat_turn(ctx, span_pk, s, start_ns, end_ns).await?,
        SpanKindClass::ExecuteTool => upsert_tool_call(ctx, span_pk, s, start_ns, end_ns, duration_ns, status_code).await?,
        SpanKindClass::ExternalTool => upsert_external_tool_call(ctx, span_pk, s, start_ns, end_ns, duration_ns).await?,
        SpanKindClass::Other => {}
    }

    resolve_projection_pointers(ctx, span_pk).await?;
    forward_resolve_descendants(ctx, span_pk).await?;

    let conv_id = effective_conversation_id(ctx.pool, span_pk).await?;
    let model = attr_str(&s.attributes, "gen_ai.request.model")
        .or_else(|| attr_str(&s.attributes, "gen_ai.response.model"))
        .map(|s| s.to_string());
    if let Some(cid) = &conv_id {
        upsert_session_for_span(ctx, cid, start_ns, end_ns, model.as_deref()).await?;
    }

    derive_from_events(ctx, span_pk, s, conv_id.as_deref()).await?;

    if kind_class == SpanKindClass::Chat {
        let input = attr_i64(&s.attributes, "gen_ai.usage.input_tokens");
        let output = attr_i64(&s.attributes, "gen_ai.usage.output_tokens");
        let cache = attr_i64(&s.attributes, "gen_ai.usage.cache_read.input_tokens");
        let reasoning = attr_i64(&s.attributes, "gen_ai.usage.reasoning.output_tokens");
        if input.is_some() || output.is_some() || cache.is_some() || reasoning.is_some() {
            insert_chat_context_snapshot(ctx, span_pk, conv_id.as_deref(),
                end_ns.unwrap_or(start_ns), input, output, cache, reasoning).await?;
        }
    }

    refresh_chat_turn_tool_count_for_span(ctx, span_pk).await?;

    let action = if was_placeholder { "upgrade" } else { "insert" };
    ctx.bus.send(EventMsg {
        kind: "span".into(), entity: "span".into(),
        payload: json!({
            "action": action, "trace_id": s.trace_id, "span_id": s.span_id,
            "parent_span_id": s.parent_span_id,
            "name": s.name, "kind_class": kind_class.as_str(),
            "ingestion_state": "real",
            "span_pk": span_pk,
        }),
    });
    ctx.bus.send(EventMsg {
        kind: "trace".into(), entity: "trace".into(),
        payload: json!({
            "action": action, "trace_id": s.trace_id, "span_id": s.span_id,
            "ingestion_state": "real", "upgraded": was_placeholder,
        }),
    });
    emit_projection_event(ctx, span_pk, kind_class).await?;

    debug!(span_pk, name=%s.name, "normalized span");
    Ok(())
}

async fn ensure_placeholder(ctx: &NormalizeCtx<'_>, trace_id: &str, span_id: &str) -> sqlx::Result<()> {
    let inserted: Option<i64> = sqlx::query_scalar(
        "INSERT INTO spans(trace_id, span_id, name, attributes_json, ingestion_state, first_seen_raw_id, last_seen_raw_id) \
         VALUES(?, ?, '', '{}', 'placeholder', ?, ?) \
         ON CONFLICT(trace_id, span_id) DO NOTHING \
         RETURNING span_pk"
    ).bind(trace_id).bind(span_id).bind(ctx.raw_record_id).bind(ctx.raw_record_id)
     .fetch_optional(ctx.pool).await?;
    let Some(pk) = inserted else { return Ok(()); };
    ctx.bus.send(EventMsg {
        kind: "span".into(), entity: "placeholder".into(),
        payload: json!({"action":"insert", "trace_id": trace_id, "span_id": span_id, "span_pk": pk}),
    });
    ctx.bus.send(EventMsg {
        kind: "trace".into(), entity: "trace".into(),
        payload: json!({
            "action":"placeholder", "trace_id": trace_id, "span_id": span_id,
            "ingestion_state": "placeholder", "upgraded": false,
        }),
    });
    Ok(())
}

async fn upsert_agent_run(
    ctx: &NormalizeCtx<'_>, span_pk: i64, s: &SpanEnvelope,
    start_ns: i64, end_ns: Option<i64>, duration_ns: Option<i64>,
) -> anyhow::Result<()> {
    let agent_name = attr_str(&s.attributes, "gen_ai.agent.name").map(String::from)
        .or_else(|| s.name.strip_prefix("invoke_agent ").map(String::from));
    let agent_id = attr_str(&s.attributes, "gen_ai.agent.id").map(String::from);
    let agent_version = attr_str(&s.attributes, "gen_ai.agent.version").map(String::from);
    let conv_id = attr_str(&s.attributes, "gen_ai.conversation.id").map(String::from);
    sqlx::query(
        "INSERT INTO agent_runs(span_pk, conversation_id, agent_id, agent_name, agent_version, \
            start_unix_ns, end_unix_ns, duration_ns) \
         VALUES(?,?,?,?,?,?,?,?) \
         ON CONFLICT(span_pk) DO UPDATE SET \
            agent_id = COALESCE(excluded.agent_id, agent_runs.agent_id), \
            agent_name = COALESCE(excluded.agent_name, agent_runs.agent_name), \
            agent_version = COALESCE(excluded.agent_version, agent_runs.agent_version), \
            conversation_id = COALESCE(excluded.conversation_id, agent_runs.conversation_id), \
            start_unix_ns = excluded.start_unix_ns, \
            end_unix_ns = excluded.end_unix_ns, \
            duration_ns = excluded.duration_ns"
    )
    .bind(span_pk).bind(conv_id).bind(agent_id).bind(agent_name).bind(agent_version)
    .bind(start_ns).bind(end_ns).bind(duration_ns)
    .execute(ctx.pool).await?;
    Ok(())
}

async fn upsert_chat_turn(
    ctx: &NormalizeCtx<'_>, span_pk: i64, s: &SpanEnvelope,
    start_ns: i64, end_ns: Option<i64>,
) -> anyhow::Result<()> {
    let conv_id = attr_str(&s.attributes, "gen_ai.conversation.id").map(String::from);
    let interaction_id = attr_str(&s.attributes, "github.copilot.interaction_id").map(String::from);
    let turn_id = attr_str(&s.attributes, "github.copilot.turn_id").map(String::from);
    let model = attr_str(&s.attributes, "gen_ai.request.model").map(String::from)
        .or_else(|| attr_str(&s.attributes, "gen_ai.response.model").map(String::from));
    let input = attr_i64(&s.attributes, "gen_ai.usage.input_tokens");
    let output = attr_i64(&s.attributes, "gen_ai.usage.output_tokens");
    let cache = attr_i64(&s.attributes, "gen_ai.usage.cache_read.input_tokens");
    let reasoning = attr_i64(&s.attributes, "gen_ai.usage.reasoning.output_tokens");
    sqlx::query(
        "INSERT INTO chat_turns(span_pk, conversation_id, interaction_id, turn_id, model, \
            input_tokens, output_tokens, cache_read_tokens, reasoning_tokens, \
            start_unix_ns, end_unix_ns) \
         VALUES(?,?,?,?,?,?,?,?,?,?,?) \
         ON CONFLICT(span_pk) DO UPDATE SET \
            conversation_id = COALESCE(excluded.conversation_id, chat_turns.conversation_id), \
            interaction_id = COALESCE(excluded.interaction_id, chat_turns.interaction_id), \
            turn_id = COALESCE(excluded.turn_id, chat_turns.turn_id), \
            model = COALESCE(excluded.model, chat_turns.model), \
            input_tokens = COALESCE(excluded.input_tokens, chat_turns.input_tokens), \
            output_tokens = COALESCE(excluded.output_tokens, chat_turns.output_tokens), \
            cache_read_tokens = COALESCE(excluded.cache_read_tokens, chat_turns.cache_read_tokens), \
            reasoning_tokens = COALESCE(excluded.reasoning_tokens, chat_turns.reasoning_tokens), \
            start_unix_ns = excluded.start_unix_ns, \
            end_unix_ns = excluded.end_unix_ns"
    )
    .bind(span_pk).bind(conv_id).bind(interaction_id).bind(turn_id).bind(model)
    .bind(input).bind(output).bind(cache).bind(reasoning)
    .bind(start_ns).bind(end_ns)
    .execute(ctx.pool).await?;
    Ok(())
}

async fn upsert_tool_call(
    ctx: &NormalizeCtx<'_>, span_pk: i64, s: &SpanEnvelope,
    start_ns: i64, end_ns: Option<i64>, duration_ns: Option<i64>, status_code: Option<i64>,
) -> anyhow::Result<()> {
    let call_id = attr_str(&s.attributes, "gen_ai.tool.call.id").map(String::from);
    let tool_name = attr_str(&s.attributes, "gen_ai.tool.name").map(String::from);
    let tool_type = attr_str(&s.attributes, "gen_ai.tool.type").map(String::from);
    let conv_id = attr_str(&s.attributes, "gen_ai.conversation.id").map(String::from);
    sqlx::query(
        "INSERT INTO tool_calls(span_pk, call_id, tool_name, tool_type, conversation_id, \
            start_unix_ns, end_unix_ns, duration_ns, status_code) \
         VALUES(?,?,?,?,?,?,?,?,?) \
         ON CONFLICT(span_pk) DO UPDATE SET \
            call_id = COALESCE(excluded.call_id, tool_calls.call_id), \
            tool_name = COALESCE(excluded.tool_name, tool_calls.tool_name), \
            tool_type = COALESCE(excluded.tool_type, tool_calls.tool_type), \
            conversation_id = COALESCE(excluded.conversation_id, tool_calls.conversation_id), \
            start_unix_ns = excluded.start_unix_ns, \
            end_unix_ns = excluded.end_unix_ns, \
            duration_ns = excluded.duration_ns, \
            status_code = excluded.status_code"
    )
    .bind(span_pk).bind(&call_id).bind(tool_name).bind(tool_type).bind(conv_id)
    .bind(start_ns).bind(end_ns).bind(duration_ns).bind(status_code)
    .execute(ctx.pool).await?;

    if let Some(cid) = &call_id {
        sqlx::query(
            "UPDATE external_tool_calls SET paired_tool_call_pk = (SELECT tool_call_pk FROM tool_calls WHERE span_pk = ?) \
             WHERE call_id = ? AND paired_tool_call_pk IS NULL"
        ).bind(span_pk).bind(cid).execute(ctx.pool).await?;
    }
    Ok(())
}

async fn upsert_external_tool_call(
    ctx: &NormalizeCtx<'_>, span_pk: i64, s: &SpanEnvelope,
    start_ns: i64, end_ns: Option<i64>, duration_ns: Option<i64>,
) -> anyhow::Result<()> {
    let call_id = attr_str(&s.attributes, "github.copilot.external_tool.call_id")
        .or_else(|| attr_str(&s.attributes, "gen_ai.tool.call.id"))
        .map(String::from);
    let tool_name = attr_str(&s.attributes, "github.copilot.external_tool.name")
        .or_else(|| attr_str(&s.attributes, "gen_ai.tool.name"))
        .map(String::from);
    let conv_id = attr_str(&s.attributes, "gen_ai.conversation.id").map(String::from);
    let paired: Option<i64> = if let Some(cid) = &call_id {
        sqlx::query_scalar("SELECT tool_call_pk FROM tool_calls WHERE call_id = ? LIMIT 1")
            .bind(cid).fetch_optional(ctx.pool).await?
    } else { None };
    sqlx::query(
        "INSERT INTO external_tool_calls(span_pk, call_id, tool_name, paired_tool_call_pk, \
            conversation_id, start_unix_ns, end_unix_ns, duration_ns) \
         VALUES(?,?,?,?,?,?,?,?) \
         ON CONFLICT(span_pk) DO UPDATE SET \
            call_id = COALESCE(excluded.call_id, external_tool_calls.call_id), \
            tool_name = COALESCE(excluded.tool_name, external_tool_calls.tool_name), \
            paired_tool_call_pk = COALESCE(external_tool_calls.paired_tool_call_pk, excluded.paired_tool_call_pk), \
            conversation_id = COALESCE(excluded.conversation_id, external_tool_calls.conversation_id), \
            start_unix_ns = excluded.start_unix_ns, \
            end_unix_ns = excluded.end_unix_ns, \
            duration_ns = excluded.duration_ns"
    )
    .bind(span_pk).bind(call_id).bind(tool_name).bind(paired).bind(conv_id)
    .bind(start_ns).bind(end_ns).bind(duration_ns)
    .execute(ctx.pool).await?;
    Ok(())
}

#[derive(Default, Debug, sqlx::FromRow)]
struct AncestorRow {
    agent_run_pk: Option<i64>,
    chat_turn_pk: Option<i64>,
    #[allow(dead_code)]
    chat_turn_span_pk: Option<i64>,
    tool_call_pk: Option<i64>,
    #[allow(dead_code)]
    tool_call_span_pk: Option<i64>,
    nearest_invoker_span_pk: Option<i64>,
    conv_id: Option<String>,
}

async fn walk_ancestors(pool: &SqlitePool, span_pk: i64) -> sqlx::Result<AncestorRow> {
    let row: Option<AncestorRow> = sqlx::query_as(
        r#"
        WITH RECURSIVE ancestors(span_pk, parent_span_id, trace_id, depth, attributes_json) AS (
            SELECT span_pk, parent_span_id, trace_id, 0, attributes_json
              FROM spans WHERE span_pk = ?1
            UNION ALL
            SELECT s.span_pk, s.parent_span_id, s.trace_id, a.depth + 1, s.attributes_json
              FROM spans s JOIN ancestors a
                ON s.trace_id = a.trace_id AND s.span_id = a.parent_span_id
              WHERE a.depth < 64
        )
        SELECT
          (SELECT ar.agent_run_pk FROM ancestors a JOIN agent_runs ar ON ar.span_pk = a.span_pk
             WHERE a.depth > 0 ORDER BY a.depth ASC LIMIT 1) AS agent_run_pk,
          (SELECT ct.turn_pk FROM ancestors a JOIN chat_turns ct ON ct.span_pk = a.span_pk
             WHERE a.depth > 0 ORDER BY a.depth ASC LIMIT 1) AS chat_turn_pk,
          (SELECT a.span_pk FROM ancestors a JOIN chat_turns ct ON ct.span_pk = a.span_pk
             WHERE a.depth > 0 ORDER BY a.depth ASC LIMIT 1) AS chat_turn_span_pk,
          (SELECT tc.tool_call_pk FROM ancestors a JOIN tool_calls tc ON tc.span_pk = a.span_pk
             WHERE a.depth > 0 ORDER BY a.depth ASC LIMIT 1) AS tool_call_pk,
          (SELECT a.span_pk FROM ancestors a JOIN tool_calls tc ON tc.span_pk = a.span_pk
             WHERE a.depth > 0 ORDER BY a.depth ASC LIMIT 1) AS tool_call_span_pk,
          (SELECT a.span_pk FROM ancestors a
             WHERE a.depth > 0 AND (
               EXISTS (SELECT 1 FROM tool_calls tc WHERE tc.span_pk = a.span_pk) OR
               EXISTS (SELECT 1 FROM chat_turns ct WHERE ct.span_pk = a.span_pk)
             )
             ORDER BY a.depth ASC LIMIT 1) AS nearest_invoker_span_pk,
          (SELECT json_extract(a.attributes_json, '$."gen_ai.conversation.id"')
             FROM ancestors a
             WHERE json_extract(a.attributes_json, '$."gen_ai.conversation.id"') IS NOT NULL
             ORDER BY a.depth ASC LIMIT 1) AS conv_id
        "#,
    ).bind(span_pk).fetch_optional(pool).await?;
    Ok(row.unwrap_or_default())
}

async fn resolve_projection_pointers(ctx: &NormalizeCtx<'_>, span_pk: i64) -> sqlx::Result<()> {
    let res = walk_ancestors(ctx.pool, span_pk).await?;

    sqlx::query(
        "UPDATE agent_runs SET \
            parent_agent_run_pk = COALESCE(parent_agent_run_pk, ?), \
            parent_span_pk = COALESCE(parent_span_pk, ?), \
            conversation_id = COALESCE(conversation_id, ?) \
         WHERE span_pk = ?"
    ).bind(res.agent_run_pk).bind(res.nearest_invoker_span_pk).bind(&res.conv_id).bind(span_pk)
     .execute(ctx.pool).await?;

    sqlx::query(
        "UPDATE chat_turns SET \
            agent_run_pk = COALESCE(agent_run_pk, ?), \
            conversation_id = COALESCE(conversation_id, ?) \
         WHERE span_pk = ?"
    ).bind(res.agent_run_pk).bind(&res.conv_id).bind(span_pk)
     .execute(ctx.pool).await?;

    sqlx::query(
        "UPDATE tool_calls SET \
            agent_run_pk = COALESCE(agent_run_pk, ?), \
            chat_turn_pk = COALESCE(chat_turn_pk, ?), \
            conversation_id = COALESCE(conversation_id, ?) \
         WHERE span_pk = ?"
    ).bind(res.agent_run_pk).bind(res.chat_turn_pk).bind(&res.conv_id).bind(span_pk)
     .execute(ctx.pool).await?;

    sqlx::query(
        "UPDATE external_tool_calls SET \
            agent_run_pk = COALESCE(agent_run_pk, ?), \
            chat_turn_pk = COALESCE(chat_turn_pk, ?), \
            conversation_id = COALESCE(conversation_id, ?) \
         WHERE span_pk = ?"
    ).bind(res.agent_run_pk).bind(res.chat_turn_pk).bind(&res.conv_id).bind(span_pk)
     .execute(ctx.pool).await?;

    sqlx::query(
        "UPDATE hook_invocations SET \
            agent_run_pk = COALESCE(agent_run_pk, ?), \
            chat_turn_pk = COALESCE(chat_turn_pk, ?), \
            tool_call_pk = COALESCE(tool_call_pk, ?), \
            conversation_id = COALESCE(conversation_id, ?) \
         WHERE span_pk = ?"
    ).bind(res.agent_run_pk).bind(res.chat_turn_pk).bind(res.tool_call_pk).bind(&res.conv_id).bind(span_pk)
     .execute(ctx.pool).await?;

    sqlx::query(
        "UPDATE skill_invocations SET \
            agent_run_pk = COALESCE(agent_run_pk, ?), \
            chat_turn_pk = COALESCE(chat_turn_pk, ?), \
            conversation_id = COALESCE(conversation_id, ?) \
         WHERE span_pk = ?"
    ).bind(res.agent_run_pk).bind(res.chat_turn_pk).bind(&res.conv_id).bind(span_pk)
     .execute(ctx.pool).await?;

    // context_snapshots are derived from the chat span itself, so the
    // matching chat_turn lives at depth 0 of the ancestor walk — which
    // `walk_ancestors` excludes (depth > 0). Look it up directly instead
    // so chat_turn_pk gets populated reliably.
    sqlx::query(
        "UPDATE context_snapshots SET \
            chat_turn_pk = COALESCE(chat_turn_pk, \
                (SELECT turn_pk FROM chat_turns WHERE span_pk = ?1)), \
            conversation_id = COALESCE(conversation_id, ?2) \
         WHERE span_pk = ?1"
    ).bind(span_pk).bind(&res.conv_id)
     .execute(ctx.pool).await?;

    Ok(())
}

async fn forward_resolve_descendants(ctx: &NormalizeCtx<'_>, ancestor_span_pk: i64) -> sqlx::Result<()> {
    let descendants: Vec<(i64,)> = sqlx::query_as(
        r#"
        WITH RECURSIVE rec(span_pk, span_id, trace_id, depth) AS (
            SELECT span_pk, span_id, trace_id, 0 FROM spans WHERE span_pk = ?1
            UNION ALL
            SELECT s.span_pk, s.span_id, s.trace_id, r.depth + 1
              FROM spans s JOIN rec r
                ON s.trace_id = r.trace_id AND s.parent_span_id = r.span_id
              WHERE r.depth < 128
        )
        SELECT span_pk FROM rec WHERE span_pk != ?1
        "#,
    ).bind(ancestor_span_pk).fetch_all(ctx.pool).await?;
    for (pk,) in descendants {
        resolve_projection_pointers(ctx, pk).await?;
    }
    Ok(())
}

async fn effective_conversation_id(pool: &SqlitePool, span_pk: i64) -> sqlx::Result<Option<String>> {
    let row: Option<(Option<String>,)> = sqlx::query_as(
        r#"
        WITH RECURSIVE chain(span_pk, parent_span_id, trace_id, depth, attributes_json) AS (
            SELECT span_pk, parent_span_id, trace_id, 0, attributes_json
              FROM spans WHERE span_pk = ?1
            UNION ALL
            SELECT s.span_pk, s.parent_span_id, s.trace_id, c.depth + 1, s.attributes_json
              FROM spans s JOIN chain c
                ON s.trace_id = c.trace_id AND s.span_id = c.parent_span_id
              WHERE c.depth < 64
        )
        SELECT json_extract(attributes_json, '$."gen_ai.conversation.id"') FROM chain
         WHERE json_extract(attributes_json, '$."gen_ai.conversation.id"') IS NOT NULL
         ORDER BY depth ASC LIMIT 1
        "#,
    ).bind(span_pk).fetch_optional(pool).await?;
    Ok(row.and_then(|(c,)| c))
}

async fn upsert_session_for_span(
    ctx: &NormalizeCtx<'_>, conv_id: &str,
    start_ns: i64, end_ns: Option<i64>, model: Option<&str>,
) -> anyhow::Result<()> {
    let last_ns = end_ns.unwrap_or(start_ns);
    sqlx::query(
        "INSERT INTO sessions(conversation_id, first_seen_ns, last_seen_ns, latest_model) \
         VALUES(?,?,?,?) \
         ON CONFLICT(conversation_id) DO UPDATE SET \
            first_seen_ns = MIN(COALESCE(sessions.first_seen_ns, excluded.first_seen_ns), excluded.first_seen_ns), \
            last_seen_ns = MAX(COALESCE(sessions.last_seen_ns, excluded.last_seen_ns), excluded.last_seen_ns), \
            latest_model = COALESCE(excluded.latest_model, sessions.latest_model)"
    )
    .bind(conv_id).bind(start_ns).bind(last_ns).bind(model)
    .execute(ctx.pool).await?;

    sqlx::query(
        "UPDATE sessions SET \
            chat_turn_count = (SELECT COUNT(*) FROM chat_turns WHERE conversation_id = ?1), \
            tool_call_count = (SELECT COUNT(*) FROM tool_calls WHERE conversation_id = ?1), \
            agent_run_count = (SELECT COUNT(*) FROM agent_runs WHERE conversation_id = ?1) \
         WHERE conversation_id = ?1"
    ).bind(conv_id).execute(ctx.pool).await?;

    ctx.bus.send(EventMsg {
        kind: "derived".into(), entity: "session".into(),
        payload: json!({"action": "update", "conversation_id": conv_id, "latest_model": model}),
    });
    Ok(())
}

async fn derive_from_events(
    ctx: &NormalizeCtx<'_>, span_pk: i64, s: &SpanEnvelope, conv_id: Option<&str>,
) -> anyhow::Result<()> {
    let mut emitted = false;
    for ev in &s.events {
        let t_ns = ev.time.to_unix_nanos();
        match ev.name.as_str() {
            "github.copilot.hook.start" => {
                let inv = attr_str(&ev.attributes, "github.copilot.hook.invocation_id");
                let typ = attr_str(&ev.attributes, "github.copilot.hook.type");
                sqlx::query(
                    "INSERT INTO hook_invocations(invocation_id, hook_type, span_pk, conversation_id, start_unix_ns) \
                     VALUES(?,?,?,?,?) \
                     ON CONFLICT(invocation_id) DO UPDATE SET \
                        start_unix_ns = excluded.start_unix_ns, \
                        conversation_id = COALESCE(hook_invocations.conversation_id, excluded.conversation_id)"
                ).bind(inv).bind(typ).bind(span_pk).bind(conv_id).bind(t_ns)
                 .execute(ctx.pool).await?;
                emitted = true;
            }
            "github.copilot.hook.end" => {
                let inv = attr_str(&ev.attributes, "github.copilot.hook.invocation_id");
                let typ = attr_str(&ev.attributes, "github.copilot.hook.type");
                sqlx::query(
                    "INSERT INTO hook_invocations(invocation_id, hook_type, span_pk, conversation_id, end_unix_ns) \
                     VALUES(?,?,?,?,?) \
                     ON CONFLICT(invocation_id) DO UPDATE SET \
                        end_unix_ns = excluded.end_unix_ns, \
                        conversation_id = COALESCE(hook_invocations.conversation_id, excluded.conversation_id), \
                        duration_ns = CASE WHEN start_unix_ns IS NOT NULL THEN excluded.end_unix_ns - start_unix_ns ELSE NULL END"
                ).bind(inv).bind(typ).bind(span_pk).bind(conv_id).bind(t_ns)
                 .execute(ctx.pool).await?;
                emitted = true;
            }
            "github.copilot.skill.invoked" => {
                let name = attr_str(&ev.attributes, "github.copilot.skill.name");
                let path = attr_str(&ev.attributes, "github.copilot.skill.path");
                sqlx::query(
                    "INSERT INTO skill_invocations(span_pk, skill_name, skill_path, invoked_unix_ns, conversation_id) \
                     VALUES(?,?,?,?,?) \
                     ON CONFLICT(span_pk, invoked_unix_ns, skill_name) DO NOTHING"
                ).bind(span_pk).bind(name).bind(path).bind(t_ns).bind(conv_id)
                 .execute(ctx.pool).await?;
                emitted = true;
            }
            "github.copilot.session.usage_info" => {
                let token_limit = attr_i64(&ev.attributes, "github.copilot.token_limit");
                let cur = attr_i64(&ev.attributes, "github.copilot.current_tokens");
                let mlen = attr_i64(&ev.attributes, "github.copilot.messages_length");
                sqlx::query(
                    "INSERT INTO context_snapshots(span_pk, conversation_id, captured_ns, \
                     token_limit, current_tokens, messages_length, source) \
                     VALUES(?,?,?,?,?,?,'usage_info_event') \
                     ON CONFLICT(span_pk, source, captured_ns) DO UPDATE SET \
                        token_limit = COALESCE(excluded.token_limit, context_snapshots.token_limit), \
                        current_tokens = COALESCE(excluded.current_tokens, context_snapshots.current_tokens), \
                        messages_length = COALESCE(excluded.messages_length, context_snapshots.messages_length)"
                ).bind(span_pk).bind(conv_id).bind(t_ns).bind(token_limit).bind(cur).bind(mlen)
                 .execute(ctx.pool).await?;
                emitted = true;
            }
            _ => {}
        }
    }
    if emitted {
        resolve_projection_pointers(ctx, span_pk).await?;
    }
    Ok(())
}

async fn insert_chat_context_snapshot(
    ctx: &NormalizeCtx<'_>, span_pk: i64, conv_id: Option<&str>,
    captured_ns: i64,
    input: Option<i64>, output: Option<i64>, cache: Option<i64>, reasoning: Option<i64>,
) -> anyhow::Result<()> {
    // chat_turn_pk is the turn associated with the chat span itself.
    // upsert_chat_turn has already run for span_pk by the time we get
    // here, so the lookup resolves immediately.
    let chat_turn_pk: Option<i64> = sqlx::query_scalar(
        "SELECT turn_pk FROM chat_turns WHERE span_pk = ?"
    ).bind(span_pk).fetch_optional(ctx.pool).await?;
    sqlx::query(
        "INSERT INTO context_snapshots(span_pk, conversation_id, chat_turn_pk, captured_ns, \
         input_tokens, output_tokens, cache_read_tokens, reasoning_tokens, source) \
         VALUES(?,?,?,?,?,?,?,?,'chat_span') \
         ON CONFLICT(span_pk, source, captured_ns) DO UPDATE SET \
            chat_turn_pk = COALESCE(context_snapshots.chat_turn_pk, excluded.chat_turn_pk), \
            input_tokens = COALESCE(excluded.input_tokens, context_snapshots.input_tokens), \
            output_tokens = COALESCE(excluded.output_tokens, context_snapshots.output_tokens), \
            cache_read_tokens = COALESCE(excluded.cache_read_tokens, context_snapshots.cache_read_tokens), \
            reasoning_tokens = COALESCE(excluded.reasoning_tokens, context_snapshots.reasoning_tokens)"
    ).bind(span_pk).bind(conv_id).bind(chat_turn_pk).bind(captured_ns)
     .bind(input).bind(output).bind(cache).bind(reasoning)
     .execute(ctx.pool).await?;
    Ok(())
}

async fn refresh_chat_turn_tool_count_for_span(ctx: &NormalizeCtx<'_>, span_pk: i64) -> sqlx::Result<()> {
    let chat_turn_pks: Vec<(i64,)> = sqlx::query_as(
        "SELECT DISTINCT turn_pk FROM (
            SELECT turn_pk FROM chat_turns WHERE span_pk = ?1
            UNION SELECT chat_turn_pk AS turn_pk FROM tool_calls WHERE span_pk = ?1 AND chat_turn_pk IS NOT NULL
            UNION SELECT chat_turn_pk AS turn_pk FROM external_tool_calls WHERE span_pk = ?1 AND chat_turn_pk IS NOT NULL
         )"
    ).bind(span_pk).fetch_all(ctx.pool).await?;
    for (tpk,) in chat_turn_pks {
        sqlx::query(
            "UPDATE chat_turns SET tool_call_count = \
                (SELECT COUNT(*) FROM tool_calls WHERE chat_turn_pk = ?1) \
             WHERE turn_pk = ?1"
        ).bind(tpk).execute(ctx.pool).await?;
    }
    Ok(())
}

async fn emit_projection_event(ctx: &NormalizeCtx<'_>, span_pk: i64, kind: SpanKindClass) -> sqlx::Result<()> {
    match kind {
        SpanKindClass::InvokeAgent => {
            let row: Option<(i64, Option<String>, Option<String>, Option<i64>, Option<i64>)> = sqlx::query_as(
                "SELECT agent_run_pk, conversation_id, agent_name, parent_agent_run_pk, parent_span_pk \
                 FROM agent_runs WHERE span_pk = ?"
            ).bind(span_pk).fetch_optional(ctx.pool).await?;
            if let Some((pk, cid, name, parent, parent_span)) = row {
                ctx.bus.send(EventMsg {
                    kind: "derived".into(), entity: "agent_run".into(),
                    payload: json!({
                        "action":"upsert", "agent_run_pk": pk, "span_pk": span_pk,
                        "conversation_id": cid, "agent_name": name,
                        "parent_agent_run_pk": parent, "parent_span_pk": parent_span,
                    }),
                });
            }
        }
        SpanKindClass::Chat => {
            let row: Option<(i64, Option<String>, Option<i64>, Option<String>, Option<String>)> = sqlx::query_as(
                "SELECT turn_pk, conversation_id, agent_run_pk, interaction_id, turn_id \
                 FROM chat_turns WHERE span_pk = ?"
            ).bind(span_pk).fetch_optional(ctx.pool).await?;
            if let Some((pk, cid, arpk, iid, tid)) = row {
                ctx.bus.send(EventMsg {
                    kind: "derived".into(), entity: "chat_turn".into(),
                    payload: json!({
                        "action":"upsert", "turn_pk": pk, "span_pk": span_pk,
                        "conversation_id": cid, "agent_run_pk": arpk,
                        "interaction_id": iid, "turn_id": tid,
                    }),
                });
            }
        }
        SpanKindClass::ExecuteTool => {
            let row: Option<(i64, Option<String>, Option<String>, Option<i64>, Option<String>)> = sqlx::query_as(
                "SELECT tool_call_pk, tool_name, call_id, agent_run_pk, conversation_id \
                 FROM tool_calls WHERE span_pk = ?"
            ).bind(span_pk).fetch_optional(ctx.pool).await?;
            if let Some((pk, tn, cid_call, arpk, conv)) = row {
                ctx.bus.send(EventMsg {
                    kind: "derived".into(), entity: "tool_call".into(),
                    payload: json!({
                        "action":"upsert", "tool_call_pk": pk, "span_pk": span_pk,
                        "tool_name": tn, "call_id": cid_call,
                        "agent_run_pk": arpk,
                        "conversation_id": conv,
                    }),
                });
            }
        }
        SpanKindClass::ExternalTool | SpanKindClass::Other => {}
    }
    Ok(())
}

async fn normalize_metric(ctx: &NormalizeCtx<'_>, m: &MetricEnvelope) -> anyhow::Result<()> {
    let resource_json = m.resource.as_ref().map(|r| serde_json::to_string(r)).transpose()?;
    let (scope_name, scope_version) = match &m.instrumentation_scope {
        Some(s) => (s.name.clone(), s.version.clone()),
        None => (None, None),
    };
    for dp in &m.data_points {
        let attrs = serde_json::to_string(&dp.attributes)?;
        let value = serde_json::to_string(&dp.value)?;
        let start = dp.start_time.as_ref().map(|t| t.to_unix_nanos());
        let end = dp.end_time.as_ref().map(|t| t.to_unix_nanos());
        sqlx::query(
            "INSERT INTO metric_points(raw_record_id, metric_name, description, unit, start_unix_ns, end_unix_ns, attributes_json, value_json, resource_json, scope_name, scope_version) \
             VALUES(?,?,?,?,?,?,?,?,?,?,?)"
        )
        .bind(ctx.raw_record_id).bind(&m.name).bind(&m.description).bind(&m.unit)
        .bind(start).bind(end).bind(&attrs).bind(&value).bind(&resource_json)
        .bind(&scope_name).bind(&scope_version)
        .execute(ctx.pool).await?;
    }
    ctx.bus.send(EventMsg::raw("metric", json!({"name": m.name, "points": m.data_points.len()})));
    Ok(())
}
