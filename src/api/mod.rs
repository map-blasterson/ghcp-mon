//! Spans-first REST API.
//!
//! Selection key is `(trace_id, span_id)`. Domain rows are projection blocks
//! returned as part of the span detail; there are no separate turn / tool-call
//! list endpoints — clients filter the span stream by `kind_class`.

use axum::{extract::{Path, Query, State}, response::IntoResponse, Json};
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use crate::server::AppState;
use crate::error::{AppError, AppResult};
use crate::ws::EventMsg;

pub async fn healthz() -> impl IntoResponse { Json(json!({"ok": true})) }

#[derive(Deserialize, Default)]
pub struct ListQuery {
    pub limit: Option<i64>,
    pub since: Option<i64>,
    pub session: Option<String>,
    pub kind: Option<String>,    // 'invoke_agent' | 'chat' | 'execute_tool' | 'external_tool' | 'other'
    #[serde(rename = "type")] pub type_filter: Option<String>,
}

fn limit(q: &ListQuery, default: i64, max: i64) -> i64 {
    q.limit.unwrap_or(default).clamp(1, max)
}

use crate::model::SpanKindClass;

fn classify(name: &str) -> &'static str {
    SpanKindClass::from_name(name).as_str()
}

// ------------------------- sessions ------------------------------------

pub async fn list_sessions(State(s): State<AppState>, Query(q): Query<ListQuery>) -> AppResult<Json<Value>> {
    let lim = limit(&q, 50, 500);
    let since = q.since.unwrap_or(0);
    let rows: Vec<(String, Option<i64>, Option<i64>, Option<String>, i64, i64, i64)> = sqlx::query_as(
        "SELECT conversation_id, first_seen_ns, last_seen_ns, latest_model, \
                chat_turn_count, tool_call_count, agent_run_count \
         FROM sessions \
         WHERE COALESCE(last_seen_ns, 0) >= ? ORDER BY COALESCE(last_seen_ns, 0) DESC LIMIT ?"
    ).bind(since).bind(lim).fetch_all(&s.pool).await?;
    let base = crate::local_session::resolve_session_state_dir(s.session_state_dir_override.as_deref());
    let out: Vec<Value> = rows.into_iter().map(|(cid, f, l, m, ctc, tcc, arc)| {
        let local = base
            .as_deref()
            .and_then(|b| crate::local_session::read_workspace_yaml(b, &cid));
        let (name, user_named, cwd, branch) = match local {
            Some(w) => (w.name, w.user_named, w.cwd, w.branch),
            None => (None, None, None, None),
        };
        json!({
            "conversation_id": cid, "first_seen_ns": f, "last_seen_ns": l,
            "latest_model": m,
            "chat_turn_count": ctc, "tool_call_count": tcc, "agent_run_count": arc,
            "local_name": name,
            "user_named": user_named,
            "cwd": cwd,
            "branch": branch,
        })
    }).collect();
    Ok(Json(json!({"sessions": out})))
}

pub async fn get_session(State(s): State<AppState>, Path(cid): Path<String>) -> AppResult<Json<Value>> {
    let row: Option<(String, Option<i64>, Option<i64>, Option<String>, i64, i64, i64)> = sqlx::query_as(
        "SELECT conversation_id, first_seen_ns, last_seen_ns, latest_model, \
                chat_turn_count, tool_call_count, agent_run_count \
         FROM sessions WHERE conversation_id = ?"
    ).bind(&cid).fetch_optional(&s.pool).await?;
    let (cid, f, l, m, ctc, tcc, arc) = row.ok_or(AppError::NotFound)?;
    let span_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM spans WHERE json_extract(attributes_json, '$.\"gen_ai.conversation.id\"') = ?"
    ).bind(&cid).fetch_one(&s.pool).await?;
    let local = crate::local_session::resolve_session_state_dir(s.session_state_dir_override.as_deref())
        .as_deref()
        .and_then(|b| crate::local_session::read_workspace_yaml(b, &cid));
    let (name, user_named, cwd, branch) = match local {
        Some(w) => (w.name, w.user_named, w.cwd, w.branch),
        None => (None, None, None, None),
    };
    Ok(Json(json!({
        "conversation_id": cid,
        "first_seen_ns": f, "last_seen_ns": l, "latest_model": m,
        "chat_turn_count": ctc, "tool_call_count": tcc, "agent_run_count": arc,
        "span_count": span_count,
        "local_name": name,
        "user_named": user_named,
        "cwd": cwd,
        "branch": branch,
    })))
}

pub async fn delete_session(State(s): State<AppState>, Path(cid): Path<String>) -> AppResult<Json<Value>> {
    // Verify the session exists first so we can return a clean 404.
    let exists: Option<(String,)> = sqlx::query_as(
        "SELECT conversation_id FROM sessions WHERE conversation_id = ?"
    ).bind(&cid).fetch_optional(&s.pool).await?;
    if exists.is_none() {
        return Err(AppError::NotFound);
    }

    // Collect every trace_id involved in this conversation. Same seed strategy
    // as the span-tree endpoint: spans whose own attrs carry the cid, plus
    // spans referenced by any projection row tagged with the cid. A CLI
    // session = one trace, so deleting all spans in those traces is correct.
    let trace_rows: Vec<(String,)> = sqlx::query_as(
        r#"
        WITH seeds AS (
            SELECT trace_id FROM spans
             WHERE json_extract(attributes_json, '$."gen_ai.conversation.id"') = ?1
            UNION
            SELECT s.trace_id FROM spans s
              JOIN agent_runs ar ON ar.span_pk = s.span_pk WHERE ar.conversation_id = ?1
            UNION
            SELECT s.trace_id FROM spans s
              JOIN chat_turns ct ON ct.span_pk = s.span_pk WHERE ct.conversation_id = ?1
            UNION
            SELECT s.trace_id FROM spans s
              JOIN tool_calls tc ON tc.span_pk = s.span_pk WHERE tc.conversation_id = ?1
        )
        SELECT DISTINCT trace_id FROM seeds
        "#,
    ).bind(&cid).fetch_all(&s.pool).await?;
    let trace_ids: Vec<String> = trace_rows.into_iter().map(|(t,)| t).collect();

    let mut tx = s.pool.begin().await?;

    // Delete all spans in matching traces. ON DELETE CASCADE on span_pk wipes
    // span_events plus the chat_turns/tool_calls/agent_runs/external_tool_calls/
    // hook_invocations/skill_invocations projection rows tied to those spans.
    if !trace_ids.is_empty() {
        let placeholders = trace_ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        let q = format!("DELETE FROM spans WHERE trace_id IN ({placeholders})");
        let mut qq = sqlx::query(&q);
        for t in &trace_ids { qq = qq.bind(t); }
        qq.execute(&mut *tx).await?;
    }

    // Clean up rows that are still tagged with the conversation_id but whose
    // span_pk was nulled (context_snapshots/hook_invocations/skill_invocations
    // use ON DELETE SET NULL on span_pk in some cases). Also belt-and-braces
    // for projection rows whose span row was already gone before this delete.
    sqlx::query("DELETE FROM context_snapshots WHERE conversation_id = ?").bind(&cid).execute(&mut *tx).await?;
    sqlx::query("DELETE FROM hook_invocations WHERE conversation_id = ?").bind(&cid).execute(&mut *tx).await?;
    sqlx::query("DELETE FROM skill_invocations WHERE conversation_id = ?").bind(&cid).execute(&mut *tx).await?;
    sqlx::query("DELETE FROM external_tool_calls WHERE conversation_id = ?").bind(&cid).execute(&mut *tx).await?;
    sqlx::query("DELETE FROM tool_calls WHERE conversation_id = ?").bind(&cid).execute(&mut *tx).await?;
    sqlx::query("DELETE FROM chat_turns WHERE conversation_id = ?").bind(&cid).execute(&mut *tx).await?;
    sqlx::query("DELETE FROM agent_runs WHERE conversation_id = ?").bind(&cid).execute(&mut *tx).await?;
    sqlx::query("DELETE FROM sessions WHERE conversation_id = ?").bind(&cid).execute(&mut *tx).await?;

    tx.commit().await?;

    s.bus.send(EventMsg {
        kind: "derived".into(), entity: "session".into(),
        payload: json!({"action": "delete", "conversation_id": cid}),
    });

    Ok(Json(json!({"deleted": true, "conversation_id": cid, "trace_count": trace_ids.len()})))
}

// ------------------------- span tree -----------------------------------

pub async fn get_session_span_tree(State(s): State<AppState>, Path(cid): Path<String>) -> AppResult<Json<Value>> {
    // All spans involved in this conversation. We collect them via projections
    // (any span_pk that has a chat_turns/tool_calls/agent_runs row tagged with
    // this conversation_id, plus their ancestors). Simpler: any span whose
    // attributes carry the cid OR is reachable via projection conv tag, OR is
    // an ancestor/descendant of one of those. For now, gather all spans whose
    // own attrs carry the cid, plus descendants of those, plus their ancestors.
    let rows: Vec<(i64, String, String, Option<String>, String, Option<i64>, Option<i64>, String, String)> = sqlx::query_as(
        r#"
        WITH RECURSIVE
        seeds AS (
            SELECT span_pk, trace_id, span_id, parent_span_id FROM spans
             WHERE json_extract(attributes_json, '$."gen_ai.conversation.id"') = ?1
            UNION
            SELECT s.span_pk, s.trace_id, s.span_id, s.parent_span_id FROM spans s
             JOIN agent_runs ar ON ar.span_pk = s.span_pk WHERE ar.conversation_id = ?1
            UNION
            SELECT s.span_pk, s.trace_id, s.span_id, s.parent_span_id FROM spans s
             JOIN chat_turns ct ON ct.span_pk = s.span_pk WHERE ct.conversation_id = ?1
            UNION
            SELECT s.span_pk, s.trace_id, s.span_id, s.parent_span_id FROM spans s
             JOIN tool_calls tc ON tc.span_pk = s.span_pk WHERE tc.conversation_id = ?1
        ),
        -- Include every span that shares a trace_id with any seed. Tool
        -- spans don't carry gen_ai.conversation.id themselves, and may
        -- sit under an orphan placeholder (e.g. a sub-agent's
        -- invoke_agent task span that hasn't landed yet). Walking
        -- ancestors+descendants from seeds misses them in that
        -- transient state because placeholders carry no parent_span_id.
        -- A CLI session = one trace, so trace-scoped union is correct
        -- and robust against orphan placeholders.
        trace_ids AS (
            SELECT DISTINCT trace_id FROM seeds
        ),
        all_pks AS (
            SELECT s.span_pk FROM spans s JOIN trace_ids t ON t.trace_id = s.trace_id
        )
        SELECT s.span_pk, s.trace_id, s.span_id, s.parent_span_id, s.name,
               s.start_unix_ns, s.end_unix_ns, s.ingestion_state, s.attributes_json
          FROM spans s WHERE s.span_pk IN (SELECT span_pk FROM all_pks)
          ORDER BY COALESCE(s.start_unix_ns, 0) ASC
        "#,
    ).bind(&cid).fetch_all(&s.pool).await?;

    // Projection lookups by span_pk.
    let pks: Vec<i64> = rows.iter().map(|r| r.0).collect();
    let proj = load_projections(&s.pool, &pks).await?;

    // Build nodes keyed by span_id; assemble tree.
    #[derive(Clone)]
    struct Node {
        v: Value,
        children: Vec<String>,
    }
    let mut nodes: BTreeMap<String, Node> = BTreeMap::new();
    let mut child_of: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (pk, tr, sp, parent, name, st, en, state, _attrs) in &rows {
        let node_v = json!({
            "span_pk": pk, "trace_id": tr, "span_id": sp, "parent_span_id": parent,
            "name": name, "kind_class": classify(name),
            "ingestion_state": state,
            "start_unix_ns": st, "end_unix_ns": en,
            "projection": proj.get(pk).cloned().unwrap_or(json!({})),
            "children": Value::Array(vec![]),
        });
        nodes.insert(sp.clone(), Node { v: node_v, children: vec![] });
        if let Some(p) = parent {
            child_of.entry(p.clone()).or_default().push(sp.clone());
        }
    }
    for (parent_id, kids) in &child_of {
        if let Some(n) = nodes.get_mut(parent_id) {
            n.children.extend(kids.clone());
        }
    }
    // Sort children newest-first; placeholder/null-start entries float to top.
    let start_by_sid: BTreeMap<String, Option<i64>> =
        rows.iter().map(|r| (r.2.clone(), r.5)).collect();
    for n in nodes.values_mut() {
        n.children.sort_by(|a, b| {
            let sa = start_by_sid.get(a).copied().flatten();
            let sb = start_by_sid.get(b).copied().flatten();
            match (sa, sb) {
                (None, None) => std::cmp::Ordering::Equal,
                (None, Some(_)) => std::cmp::Ordering::Less,
                (Some(_), None) => std::cmp::Ordering::Greater,
                (Some(x), Some(y)) => y.cmp(&x),
            }
        });
    }
    // Find roots: nodes whose parent isn't in our set or is None.
    // Sort roots newest-first by start time, with placeholder-only roots
    // (start_unix_ns NULL) floated to the very top so freshly-arrived
    // traces don't sink below older timestamped ones.
    let mut roots: Vec<String> = Vec::new();
    for (sid, _) in &nodes {
        let parent = rows.iter().find(|r| r.2 == *sid).and_then(|r| r.3.clone());
        match parent {
            None => roots.push(sid.clone()),
            Some(p) if !nodes.contains_key(&p) => roots.push(sid.clone()),
            _ => {}
        }
    }
    roots.sort_by(|a, b| {
        let sa = rows.iter().find(|r| &r.2 == a).and_then(|r| r.5);
        let sb = rows.iter().find(|r| &r.2 == b).and_then(|r| r.5);
        match (sa, sb) {
            (None, None) => std::cmp::Ordering::Equal,
            (None, Some(_)) => std::cmp::Ordering::Less,    // placeholder first
            (Some(_), None) => std::cmp::Ordering::Greater,
            (Some(x), Some(y)) => y.cmp(&x),                // newest first
        }
    });

    fn build(sid: &str, nodes: &BTreeMap<String, Node>) -> Value {
        let n = match nodes.get(sid) { Some(n) => n, None => return Value::Null };
        let kids: Vec<Value> = n.children.iter().map(|c| build(c, nodes)).collect();
        let mut v = n.v.clone();
        if let Some(obj) = v.as_object_mut() {
            obj.insert("children".into(), Value::Array(kids));
        }
        v
    }

    let tree: Vec<Value> = roots.iter().map(|r| build(r, &nodes)).collect();
    Ok(Json(json!({"conversation_id": cid, "tree": tree})))
}

async fn load_projections(pool: &sqlx::SqlitePool, pks: &[i64]) -> sqlx::Result<BTreeMap<i64, Value>> {
    if pks.is_empty() { return Ok(BTreeMap::new()); }
    let placeholders = pks.iter().map(|_| "?").collect::<Vec<_>>().join(",");
    let mut out: BTreeMap<i64, Value> = BTreeMap::new();

    // chat_turns
    let q = format!("SELECT span_pk, turn_pk, conversation_id, agent_run_pk, interaction_id, turn_id, model, \
        input_tokens, output_tokens, cache_read_tokens, reasoning_tokens, tool_call_count \
        FROM chat_turns WHERE span_pk IN ({placeholders})");
    let mut qq = sqlx::query_as::<_, (i64, i64, Option<String>, Option<i64>, Option<String>, Option<String>, Option<String>, Option<i64>, Option<i64>, Option<i64>, Option<i64>, i64)>(&q);
    for p in pks { qq = qq.bind(p); }
    for (sp, tp, cid, arpk, iid, tid, model, it, ot, ct, rt, tcc) in qq.fetch_all(pool).await? {
        out.entry(sp).or_insert_with(|| json!({})).as_object_mut().unwrap()
            .insert("chat_turn".into(), json!({
                "turn_pk": tp, "conversation_id": cid, "agent_run_pk": arpk,
                "interaction_id": iid, "turn_id": tid, "model": model,
                "input_tokens": it, "output_tokens": ot, "cache_read_tokens": ct, "reasoning_tokens": rt,
                "tool_call_count": tcc
            }));
    }

    // tool_calls
    let q = format!("SELECT span_pk, tool_call_pk, call_id, tool_name, tool_type, conversation_id, agent_run_pk, status_code \
        FROM tool_calls WHERE span_pk IN ({placeholders})");
    let mut qq = sqlx::query_as::<_, (i64, i64, Option<String>, Option<String>, Option<String>, Option<String>, Option<i64>, Option<i64>)>(&q);
    for p in pks { qq = qq.bind(p); }
    for (sp, tcpk, cid_call, tn, tt, conv, arpk, sc) in qq.fetch_all(pool).await? {
        out.entry(sp).or_insert_with(|| json!({})).as_object_mut().unwrap()
            .insert("tool_call".into(), json!({
                "tool_call_pk": tcpk, "call_id": cid_call,
                "tool_name": tn, "tool_type": tt, "conversation_id": conv,
                "agent_run_pk": arpk, "status_code": sc
            }));
    }

    // agent_runs
    let q = format!("SELECT span_pk, agent_run_pk, conversation_id, agent_id, agent_name, agent_version, parent_agent_run_pk, parent_span_pk \
        FROM agent_runs WHERE span_pk IN ({placeholders})");
    let mut qq = sqlx::query_as::<_, (i64, i64, Option<String>, Option<String>, Option<String>, Option<String>, Option<i64>, Option<i64>)>(&q);
    for p in pks { qq = qq.bind(p); }
    for (sp, arpk, conv, aid, an, av, parent, parent_span) in qq.fetch_all(pool).await? {
        out.entry(sp).or_insert_with(|| json!({})).as_object_mut().unwrap()
            .insert("agent_run".into(), json!({
                "agent_run_pk": arpk, "conversation_id": conv,
                "agent_id": aid, "agent_name": an, "agent_version": av,
                "parent_agent_run_pk": parent, "parent_span_pk": parent_span
            }));
    }

    // external_tool_calls
    let q = format!("SELECT span_pk, ext_pk, call_id, tool_name, paired_tool_call_pk, conversation_id, agent_run_pk \
        FROM external_tool_calls WHERE span_pk IN ({placeholders})");
    let mut qq = sqlx::query_as::<_, (i64, i64, Option<String>, Option<String>, Option<i64>, Option<String>, Option<i64>)>(&q);
    for p in pks { qq = qq.bind(p); }
    for (sp, ext, cid_call, tn, paired, conv, arpk) in qq.fetch_all(pool).await? {
        out.entry(sp).or_insert_with(|| json!({})).as_object_mut().unwrap()
            .insert("external_tool_call".into(), json!({
                "ext_pk": ext, "call_id": cid_call, "tool_name": tn,
                "paired_tool_call_pk": paired, "conversation_id": conv,
                "agent_run_pk": arpk
            }));
    }

    Ok(out)
}

// ------------------------- shared tree builder -------------------------

type SpanTreeRow = (i64, String, String, Option<String>, String, Option<i64>, Option<i64>, String);

async fn build_tree_for_rows(
    pool: &sqlx::SqlitePool,
    rows: Vec<SpanTreeRow>,
) -> sqlx::Result<Vec<Value>> {
    let pks: Vec<i64> = rows.iter().map(|r| r.0).collect();
    let proj = load_projections(pool, &pks).await?;

    #[derive(Clone)]
    struct Node {
        v: Value,
        children: Vec<String>,
    }
    let mut nodes: BTreeMap<String, Node> = BTreeMap::new();
    let mut child_of: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (pk, tr, sp, parent, name, st, en, state) in &rows {
        let node_v = json!({
            "span_pk": pk, "trace_id": tr, "span_id": sp, "parent_span_id": parent,
            "name": name, "kind_class": classify(name),
            "ingestion_state": state,
            "start_unix_ns": st, "end_unix_ns": en,
            "projection": proj.get(pk).cloned().unwrap_or(json!({})),
            "children": Value::Array(vec![]),
        });
        nodes.insert(sp.clone(), Node { v: node_v, children: vec![] });
        if let Some(p) = parent {
            child_of.entry(p.clone()).or_default().push(sp.clone());
        }
    }
    for (parent_id, kids) in &child_of {
        if let Some(n) = nodes.get_mut(parent_id) {
            n.children.extend(kids.clone());
        }
    }
    // Sort children newest-first; placeholder/null-start entries float to top.
    let start_by_sid: BTreeMap<String, Option<i64>> =
        rows.iter().map(|r| (r.2.clone(), r.5)).collect();
    for n in nodes.values_mut() {
        n.children.sort_by(|a, b| {
            let sa = start_by_sid.get(a).copied().flatten();
            let sb = start_by_sid.get(b).copied().flatten();
            match (sa, sb) {
                (None, None) => std::cmp::Ordering::Equal,
                (None, Some(_)) => std::cmp::Ordering::Less,
                (Some(_), None) => std::cmp::Ordering::Greater,
                (Some(x), Some(y)) => y.cmp(&x),
            }
        });
    }
    let mut roots: Vec<String> = Vec::new();
    for (sid, _) in &nodes {
        let parent = rows.iter().find(|r| r.2 == *sid).and_then(|r| r.3.clone());
        match parent {
            None => roots.push(sid.clone()),
            Some(p) if !nodes.contains_key(&p) => roots.push(sid.clone()),
            _ => {}
        }
    }
    fn build(sid: &str, nodes: &BTreeMap<String, Node>) -> Value {
        let n = match nodes.get(sid) { Some(n) => n, None => return Value::Null };
        let kids: Vec<Value> = n.children.iter().map(|c| build(c, nodes)).collect();
        let mut v = n.v.clone();
        if let Some(obj) = v.as_object_mut() {
            obj.insert("children".into(), Value::Array(kids));
        }
        v
    }
    Ok(roots.iter().map(|r| build(r, &nodes)).collect())
}

// ------------------------- /api/traces (live, by trace_id) -------------

pub async fn list_traces(State(s): State<AppState>, Query(q): Query<ListQuery>) -> AppResult<Json<Value>> {
    let lim = limit(&q, 50, 500);
    let since = q.since.unwrap_or(0);
    // One row per trace. Aggregates pulled across spans + raw_records (last_seen
    // is monotonic — receipt time ensures a placeholder-only trace still has
    // some last_seen even when its lone span has null timestamps).
    let rows: Vec<(String, Option<i64>, Option<i64>, i64, i64)> = sqlx::query_as(
        r#"
        SELECT
          s.trace_id,
          MIN(COALESCE(s.start_unix_ns, 0)) AS first_seen_ns,
          MAX(COALESCE(s.end_unix_ns, s.start_unix_ns, 0)) AS last_seen_ns,
          COUNT(*)                                              AS span_count,
          SUM(CASE WHEN s.ingestion_state = 'placeholder' THEN 1 ELSE 0 END) AS placeholder_count
        FROM spans s
        GROUP BY s.trace_id
        HAVING MAX(COALESCE(s.end_unix_ns, s.start_unix_ns, 0)) >= ?1
        -- Placeholder-only traces have last_seen_ns=0 because all their
        -- spans lack timestamps. Under a plain DESC sort they sink to the
        -- bottom even though they are the newest, still-arriving traces.
        -- Float them above the timestamped rows, then break ties by recency.
        ORDER BY (last_seen_ns = 0) DESC, last_seen_ns DESC
        LIMIT ?2
        "#,
    ).bind(since).bind(lim).fetch_all(&s.pool).await?;

    let mut out: Vec<Value> = Vec::with_capacity(rows.len());
    for (trace_id, first, last, span_count, placeholder_count) in rows {
        // kind_counts
        let kinds: Vec<(String, i64)> = sqlx::query_as(
            "SELECT name, COUNT(*) FROM spans WHERE trace_id = ? GROUP BY name"
        ).bind(&trace_id).fetch_all(&s.pool).await?;
        let mut kc = json!({"chat":0, "execute_tool":0, "external_tool":0, "invoke_agent":0, "other":0});
        for (name, n) in kinds {
            let k = classify(&name);
            let cur = kc.get(k).and_then(|v| v.as_i64()).unwrap_or(0);
            kc[k] = json!(cur + n);
        }
        // root: span with parent_span_id NULL or whose parent is not in this trace.
        let root: Option<(i64, String, String, Option<String>, String, String)> = sqlx::query_as(
            r#"
            SELECT s.span_pk, s.trace_id, s.span_id, s.parent_span_id, s.name, s.ingestion_state
              FROM spans s
              WHERE s.trace_id = ?1
                AND (s.parent_span_id IS NULL
                     OR NOT EXISTS (
                         SELECT 1 FROM spans p
                          WHERE p.trace_id = s.trace_id AND p.span_id = s.parent_span_id))
              ORDER BY COALESCE(s.start_unix_ns, 0) ASC, s.span_pk ASC
              LIMIT 1
            "#,
        ).bind(&trace_id).fetch_optional(&s.pool).await?;
        let root_v = root.map(|(pk, tr, sp, par, name, state)| json!({
            "span_pk": pk, "trace_id": tr, "span_id": sp, "parent_span_id": par,
            "name": name, "kind_class": classify(&name), "ingestion_state": state
        }));
        // conversation_id (any span in trace carrying gen_ai.conversation.id)
        let conv: Option<String> = sqlx::query_scalar(
            "SELECT json_extract(attributes_json, '$.\"gen_ai.conversation.id\"') \
             FROM spans WHERE trace_id = ? \
               AND json_extract(attributes_json, '$.\"gen_ai.conversation.id\"') IS NOT NULL \
             LIMIT 1"
        ).bind(&trace_id).fetch_optional(&s.pool).await?.flatten();

        out.push(json!({
            "trace_id": trace_id,
            "first_seen_ns": first,
            "last_seen_ns": last,
            "span_count": span_count,
            "placeholder_count": placeholder_count,
            "kind_counts": kc,
            "root": root_v,
            "conversation_id": conv,
        }));
    }
    Ok(Json(json!({"traces": out})))
}

pub async fn get_trace(State(s): State<AppState>, Path(trace_id): Path<String>) -> AppResult<Json<Value>> {
    let rows: Vec<SpanTreeRow> = sqlx::query_as(
        "SELECT span_pk, trace_id, span_id, parent_span_id, name, start_unix_ns, end_unix_ns, ingestion_state \
         FROM spans WHERE trace_id = ? ORDER BY COALESCE(start_unix_ns, 0) ASC"
    ).bind(&trace_id).fetch_all(&s.pool).await?;
    if rows.is_empty() { return Err(AppError::NotFound); }
    let conv: Option<String> = sqlx::query_scalar(
        "SELECT json_extract(attributes_json, '$.\"gen_ai.conversation.id\"') \
         FROM spans WHERE trace_id = ? \
           AND json_extract(attributes_json, '$.\"gen_ai.conversation.id\"') IS NOT NULL \
         LIMIT 1"
    ).bind(&trace_id).fetch_optional(&s.pool).await?.flatten();
    let tree = build_tree_for_rows(&s.pool, rows).await?;
    Ok(Json(json!({"trace_id": trace_id, "conversation_id": conv, "tree": tree})))
}

// ------------------------- /api/spans ----------------------------------

pub async fn list_spans(State(s): State<AppState>, Query(q): Query<ListQuery>) -> AppResult<Json<Value>> {
    let lim = limit(&q, 100, 1000);
    let mut sql = String::from(
        "SELECT s.span_pk, s.trace_id, s.span_id, s.parent_span_id, s.name, \
                s.start_unix_ns, s.end_unix_ns, s.ingestion_state \
         FROM spans s WHERE 1=1"
    );
    let mut binds: Vec<String> = Vec::new();
    if let Some(cid) = &q.session {
        sql.push_str(" AND (json_extract(s.attributes_json, '$.\"gen_ai.conversation.id\"') = ? \
            OR EXISTS (SELECT 1 FROM agent_runs WHERE span_pk = s.span_pk AND conversation_id = ?) \
            OR EXISTS (SELECT 1 FROM chat_turns WHERE span_pk = s.span_pk AND conversation_id = ?) \
            OR EXISTS (SELECT 1 FROM tool_calls WHERE span_pk = s.span_pk AND conversation_id = ?))");
        binds.push(cid.clone()); binds.push(cid.clone()); binds.push(cid.clone()); binds.push(cid.clone());
    }
    if let Some(kind) = &q.kind {
        // Mirrors SpanKindClass::from_name in src/model.rs.
        sql.push_str(" AND (CASE \
            WHEN s.name = 'invoke_agent' OR s.name LIKE 'invoke_agent %' THEN 'invoke_agent' \
            WHEN s.name LIKE 'chat%' THEN 'chat' \
            WHEN s.name LIKE 'execute_tool%' THEN 'execute_tool' \
            WHEN s.name LIKE 'external_tool%' THEN 'external_tool' \
            ELSE 'other' END) = ?");
        binds.push(kind.clone());
    }
    if let Some(since) = q.since {
        sql.push_str(" AND COALESCE(s.start_unix_ns, 0) >= ?");
        binds.push(since.to_string());
    }
    sql.push_str(" ORDER BY COALESCE(s.start_unix_ns, 0) DESC LIMIT ?");
    let mut qq = sqlx::query_as::<_, (i64, String, String, Option<String>, String, Option<i64>, Option<i64>, String)>(&sql);
    for b in &binds { qq = qq.bind(b); }
    qq = qq.bind(lim);
    let rows = qq.fetch_all(&s.pool).await?;
    let out: Vec<Value> = rows.into_iter()
        .map(|(pk, tr, sp, par, name, st, en, state)| json!({
            "span_pk": pk, "trace_id": tr, "span_id": sp, "parent_span_id": par,
            "name": name, "kind_class": classify(&name),
            "start_unix_ns": st, "end_unix_ns": en,
            "ingestion_state": state
        }))
        .collect();
    Ok(Json(json!({"spans": out})))
}

pub async fn get_span(State(s): State<AppState>, Path((trace_id, span_id)): Path<(String, String)>) -> AppResult<Json<Value>> {
    let row: Option<(i64, String, String, Option<String>, String, Option<i64>, Option<i64>, Option<i64>, Option<i64>, Option<String>, String, Option<String>, Option<String>, Option<String>, String)> = sqlx::query_as(
        "SELECT span_pk, trace_id, span_id, parent_span_id, name, kind, start_unix_ns, end_unix_ns, duration_ns, status_message, attributes_json, resource_json, scope_name, scope_version, ingestion_state \
         FROM spans WHERE trace_id = ? AND span_id = ?"
    ).bind(&trace_id).bind(&span_id).fetch_optional(&s.pool).await?;
    let (pk, tr, sp, par, name, kind, st, en, dur, smsg, attrs, res, scn, scv, state) = row.ok_or(AppError::NotFound)?;
    let attrs_v: Value = serde_json::from_str(&attrs).unwrap_or(Value::Null);
    let res_v: Option<Value> = res.as_deref().and_then(|s| serde_json::from_str(s).ok());

    let events: Vec<(i64, String, i64, String)> = sqlx::query_as(
        "SELECT event_pk, name, time_unix_ns, attributes_json FROM span_events WHERE span_pk = ? ORDER BY time_unix_ns ASC"
    ).bind(pk).fetch_all(&s.pool).await?;
    let events_v: Vec<Value> = events.into_iter().map(|(epk, n, t, a)| json!({
        "event_pk": epk, "name": n, "time_unix_ns": t,
        "attributes": serde_json::from_str::<Value>(&a).unwrap_or(Value::Null)
    })).collect();

    let children: Vec<(i64, String, String, String)> = sqlx::query_as(
        "SELECT span_pk, trace_id, span_id, name FROM spans WHERE trace_id = ? AND parent_span_id = ? ORDER BY COALESCE(start_unix_ns, 0) ASC"
    ).bind(&tr).bind(&sp).fetch_all(&s.pool).await?;
    let children_v: Vec<Value> = children.into_iter().map(|(cpk, ctr, csp, cname)| json!({
        "span_pk": cpk, "trace_id": ctr, "span_id": csp, "name": cname, "kind_class": classify(&cname)
    })).collect();

    let parent_v: Option<Value> = if let Some(p) = &par {
        let row: Option<(i64, String)> = sqlx::query_as(
            "SELECT span_pk, name FROM spans WHERE trace_id = ? AND span_id = ?"
        ).bind(&tr).bind(p).fetch_optional(&s.pool).await?;
        row.map(|(ppk, pname)| json!({"span_pk": ppk, "trace_id": tr, "span_id": p, "name": pname, "kind_class": classify(&pname)}))
    } else { None };

    let proj = load_projections(&s.pool, &[pk]).await?.remove(&pk).unwrap_or(json!({}));

    Ok(Json(json!({
        "span": {
            "span_pk": pk, "trace_id": tr, "span_id": sp, "parent_span_id": par,
            "name": name, "kind": kind, "kind_class": classify(&name),
            "start_unix_ns": st, "end_unix_ns": en, "duration_ns": dur,
            "status_message": smsg, "ingestion_state": state,
            "scope_name": scn, "scope_version": scv,
            "attributes": attrs_v, "resource": res_v,
        },
        "events": events_v,
        "parent": parent_v,
        "children": children_v,
        "projection": proj,
    })))
}

// ------------------------- session-scoped projections -------------------

pub async fn list_session_contexts(State(s): State<AppState>, Path(cid): Path<String>) -> AppResult<Json<Value>> {
    let rows: Vec<(i64, Option<i64>, i64, Option<i64>, Option<i64>, Option<i64>, Option<i64>, Option<i64>, Option<i64>, Option<i64>, Option<String>)> = sqlx::query_as(
        "SELECT ctx_pk, span_pk, captured_ns, token_limit, current_tokens, messages_length, \
                input_tokens, output_tokens, cache_read_tokens, reasoning_tokens, source \
         FROM context_snapshots WHERE conversation_id = ? ORDER BY captured_ns ASC"
    ).bind(&cid).fetch_all(&s.pool).await?;
    let out: Vec<Value> = rows.into_iter().map(|(pk, spk, ts, tl, ct, ml, it, ot, cr, rt, src)| json!({
        "ctx_pk": pk, "span_pk": spk, "captured_ns": ts,
        "token_limit": tl, "current_tokens": ct, "messages_length": ml,
        "input_tokens": it, "output_tokens": ot, "cache_read_tokens": cr, "reasoning_tokens": rt,
        "source": src
    })).collect();
    Ok(Json(json!({"conversation_id": cid, "context_snapshots": out})))
}

// ------------------------- raw -----------------------------------------

pub async fn list_raw(State(s): State<AppState>, Query(q): Query<ListQuery>) -> AppResult<Json<Value>> {
    let lim = limit(&q, 100, 500);
    let rows: Vec<(i64, String, String, String, Option<String>, String)> = if let Some(ref t) = q.type_filter {
        sqlx::query_as("SELECT id, received_at, source, record_type, content_type, body FROM raw_records WHERE record_type = ? ORDER BY id DESC LIMIT ?")
            .bind(t).bind(lim).fetch_all(&s.pool).await?
    } else {
        sqlx::query_as("SELECT id, received_at, source, record_type, content_type, body FROM raw_records ORDER BY id DESC LIMIT ?")
            .bind(lim).fetch_all(&s.pool).await?
    };
    let out: Vec<Value> = rows.into_iter().map(|(id, at, src, rt, ct, body)| {
        let parsed = serde_json::from_str::<Value>(&body).unwrap_or(Value::String(body));
        json!({"id": id, "received_at": at, "source": src, "record_type": rt, "content_type": ct, "body": parsed})
    }).collect();
    Ok(Json(json!({"raw": out})))
}
