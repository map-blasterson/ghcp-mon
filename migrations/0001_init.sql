-- Span-canonical schema. Spans are the source of truth; every domain row is a
-- projection keyed by `span_pk`. All projection ancestor pointers are nullable
-- and reconciled idempotently as ancestor spans land.
--
-- Out-of-order ingest is the default contract. When a span arrives whose
-- parentSpanId is unknown, a `placeholder` span row is inserted so the tree is
-- always traversable. When the real span arrives, the placeholder is upgraded
-- in place (same span_pk).

CREATE TABLE IF NOT EXISTS raw_records (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    received_at   TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    source        TEXT NOT NULL,
    record_type   TEXT NOT NULL,
    content_type  TEXT,
    body          TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_raw_records_received_at ON raw_records(received_at DESC);
CREATE INDEX IF NOT EXISTS idx_raw_records_type ON raw_records(record_type);

-- Canonical span row. ingestion_state distinguishes a real span from a
-- placeholder synthesized when a child arrived first.
CREATE TABLE IF NOT EXISTS spans (
    span_pk           INTEGER PRIMARY KEY AUTOINCREMENT,
    trace_id          TEXT NOT NULL,
    span_id           TEXT NOT NULL,
    parent_span_id    TEXT,
    name              TEXT NOT NULL,
    kind              INTEGER,
    start_unix_ns     INTEGER,             -- NULL for placeholders
    end_unix_ns       INTEGER,
    duration_ns       INTEGER,
    status_code       INTEGER,
    status_message    TEXT,
    attributes_json   TEXT NOT NULL DEFAULT '{}',
    resource_json     TEXT,
    scope_name        TEXT,
    scope_version     TEXT,
    ingestion_state   TEXT NOT NULL DEFAULT 'real',  -- 'real' | 'placeholder'
    first_seen_raw_id INTEGER REFERENCES raw_records(id) ON DELETE SET NULL,
    last_seen_raw_id  INTEGER REFERENCES raw_records(id) ON DELETE SET NULL,
    UNIQUE(trace_id, span_id)
);
CREATE INDEX IF NOT EXISTS idx_spans_parent ON spans(trace_id, parent_span_id);
CREATE INDEX IF NOT EXISTS idx_spans_name ON spans(name);
CREATE INDEX IF NOT EXISTS idx_spans_start ON spans(start_unix_ns DESC);
CREATE INDEX IF NOT EXISTS idx_spans_state ON spans(ingestion_state);

CREATE TABLE IF NOT EXISTS span_events (
    event_pk        INTEGER PRIMARY KEY AUTOINCREMENT,
    span_pk         INTEGER NOT NULL REFERENCES spans(span_pk) ON DELETE CASCADE,
    raw_record_id   INTEGER NOT NULL REFERENCES raw_records(id) ON DELETE CASCADE,
    name            TEXT NOT NULL,
    time_unix_ns    INTEGER NOT NULL,
    attributes_json TEXT NOT NULL DEFAULT '{}'
);
CREATE INDEX IF NOT EXISTS idx_span_events_span ON span_events(span_pk);
CREATE INDEX IF NOT EXISTS idx_span_events_name ON span_events(name);

CREATE TABLE IF NOT EXISTS metric_points (
    metric_pk       INTEGER PRIMARY KEY AUTOINCREMENT,
    raw_record_id   INTEGER NOT NULL REFERENCES raw_records(id) ON DELETE CASCADE,
    metric_name     TEXT NOT NULL,
    description     TEXT,
    unit            TEXT,
    start_unix_ns   INTEGER,
    end_unix_ns     INTEGER,
    attributes_json TEXT NOT NULL DEFAULT '{}',
    value_json      TEXT NOT NULL,
    resource_json   TEXT,
    scope_name      TEXT,
    scope_version   TEXT
);
CREATE INDEX IF NOT EXISTS idx_metric_points_name ON metric_points(metric_name);
CREATE INDEX IF NOT EXISTS idx_metric_points_end ON metric_points(end_unix_ns DESC);

-- Session aggregate keyed by gen_ai.conversation.id. Pure aggregate; no
-- projection pointers point at sessions.
CREATE TABLE IF NOT EXISTS sessions (
    conversation_id  TEXT PRIMARY KEY,
    first_seen_ns    INTEGER,
    last_seen_ns     INTEGER,
    latest_model     TEXT,
    span_count       INTEGER NOT NULL DEFAULT 0,
    chat_turn_count  INTEGER NOT NULL DEFAULT 0,
    tool_call_count  INTEGER NOT NULL DEFAULT 0,
    agent_run_count  INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX IF NOT EXISTS idx_sessions_last_seen ON sessions(last_seen_ns DESC);

-- One row per `invoke_agent*` span.
CREATE TABLE IF NOT EXISTS agent_runs (
    agent_run_pk         INTEGER PRIMARY KEY AUTOINCREMENT,
    span_pk              INTEGER NOT NULL UNIQUE REFERENCES spans(span_pk) ON DELETE CASCADE,
    conversation_id      TEXT,
    agent_id             TEXT,
    agent_name           TEXT,
    agent_version        TEXT,
    -- Nearest enclosing invoke_agent ancestor's agent_run_pk (NULL = root).
    parent_agent_run_pk  INTEGER REFERENCES agent_runs(agent_run_pk) ON DELETE SET NULL,
    -- The chat span that invoked this subagent (NULL for root, NULL when
    -- ancestor not yet ingested).
    parent_span_pk       INTEGER REFERENCES spans(span_pk) ON DELETE SET NULL,
    start_unix_ns        INTEGER,
    end_unix_ns          INTEGER,
    duration_ns          INTEGER
);
CREATE INDEX IF NOT EXISTS idx_agent_runs_conv ON agent_runs(conversation_id);
CREATE INDEX IF NOT EXISTS idx_agent_runs_parent_agent ON agent_runs(parent_agent_run_pk);
CREATE INDEX IF NOT EXISTS idx_agent_runs_parent_span ON agent_runs(parent_span_pk);

-- One row per `chat*` span. The chat span IS the turn.
CREATE TABLE IF NOT EXISTS chat_turns (
    turn_pk           INTEGER PRIMARY KEY AUTOINCREMENT,
    span_pk           INTEGER NOT NULL UNIQUE REFERENCES spans(span_pk) ON DELETE CASCADE,
    conversation_id   TEXT,
    agent_run_pk      INTEGER REFERENCES agent_runs(agent_run_pk) ON DELETE SET NULL,
    interaction_id    TEXT,        -- real wire id when present, else NULL (no synthesized empty string)
    turn_id           TEXT,        -- real wire id when present, else synthesized index per agent_run from start order
    model             TEXT,
    input_tokens      INTEGER,
    output_tokens     INTEGER,
    cache_read_tokens INTEGER,
    reasoning_tokens  INTEGER,
    tool_call_count   INTEGER NOT NULL DEFAULT 0,
    start_unix_ns     INTEGER,
    end_unix_ns       INTEGER
);
CREATE INDEX IF NOT EXISTS idx_chat_turns_conv ON chat_turns(conversation_id);
CREATE INDEX IF NOT EXISTS idx_chat_turns_agent_run ON chat_turns(agent_run_pk);
CREATE INDEX IF NOT EXISTS idx_chat_turns_start ON chat_turns(start_unix_ns ASC);

-- One row per `execute_tool*` span.
CREATE TABLE IF NOT EXISTS tool_calls (
    tool_call_pk     INTEGER PRIMARY KEY AUTOINCREMENT,
    span_pk          INTEGER NOT NULL UNIQUE REFERENCES spans(span_pk) ON DELETE CASCADE,
    call_id          TEXT,
    tool_name        TEXT,
    tool_type        TEXT,
    conversation_id  TEXT,
    agent_run_pk     INTEGER REFERENCES agent_runs(agent_run_pk) ON DELETE SET NULL,
    chat_turn_pk     INTEGER REFERENCES chat_turns(turn_pk) ON DELETE SET NULL,
    start_unix_ns    INTEGER,
    end_unix_ns      INTEGER,
    duration_ns      INTEGER,
    status_code      INTEGER
);
CREATE INDEX IF NOT EXISTS idx_tool_calls_call ON tool_calls(call_id);
CREATE INDEX IF NOT EXISTS idx_tool_calls_conv ON tool_calls(conversation_id);
CREATE INDEX IF NOT EXISTS idx_tool_calls_agent_run ON tool_calls(agent_run_pk);
CREATE INDEX IF NOT EXISTS idx_tool_calls_chat_turn ON tool_calls(chat_turn_pk);
CREATE INDEX IF NOT EXISTS idx_tool_calls_start ON tool_calls(start_unix_ns DESC);

CREATE TABLE IF NOT EXISTS external_tool_calls (
    ext_pk              INTEGER PRIMARY KEY AUTOINCREMENT,
    span_pk             INTEGER NOT NULL UNIQUE REFERENCES spans(span_pk) ON DELETE CASCADE,
    call_id             TEXT,
    tool_name           TEXT,
    paired_tool_call_pk INTEGER REFERENCES tool_calls(tool_call_pk) ON DELETE SET NULL,
    conversation_id     TEXT,
    agent_run_pk        INTEGER REFERENCES agent_runs(agent_run_pk) ON DELETE SET NULL,
    chat_turn_pk        INTEGER REFERENCES chat_turns(turn_pk) ON DELETE SET NULL,
    start_unix_ns       INTEGER,
    end_unix_ns         INTEGER,
    duration_ns         INTEGER
);
CREATE INDEX IF NOT EXISTS idx_ext_call ON external_tool_calls(call_id);
CREATE INDEX IF NOT EXISTS idx_ext_conv ON external_tool_calls(conversation_id);
CREATE INDEX IF NOT EXISTS idx_ext_agent_run ON external_tool_calls(agent_run_pk);
CREATE INDEX IF NOT EXISTS idx_ext_chat_turn ON external_tool_calls(chat_turn_pk);

CREATE TABLE IF NOT EXISTS hook_invocations (
    hook_pk          INTEGER PRIMARY KEY AUTOINCREMENT,
    span_pk          INTEGER REFERENCES spans(span_pk) ON DELETE CASCADE,
    invocation_id    TEXT,
    hook_type        TEXT,
    conversation_id  TEXT,
    agent_run_pk     INTEGER REFERENCES agent_runs(agent_run_pk) ON DELETE SET NULL,
    chat_turn_pk     INTEGER REFERENCES chat_turns(turn_pk) ON DELETE SET NULL,
    tool_call_pk     INTEGER REFERENCES tool_calls(tool_call_pk) ON DELETE SET NULL,
    start_unix_ns    INTEGER,
    end_unix_ns      INTEGER,
    duration_ns      INTEGER,
    UNIQUE(invocation_id)
);
CREATE INDEX IF NOT EXISTS idx_hook_type ON hook_invocations(hook_type);
CREATE INDEX IF NOT EXISTS idx_hook_conv ON hook_invocations(conversation_id);
CREATE INDEX IF NOT EXISTS idx_hook_agent_run ON hook_invocations(agent_run_pk);
CREATE INDEX IF NOT EXISTS idx_hook_chat_turn ON hook_invocations(chat_turn_pk);

CREATE TABLE IF NOT EXISTS skill_invocations (
    skill_pk         INTEGER PRIMARY KEY AUTOINCREMENT,
    span_pk          INTEGER REFERENCES spans(span_pk) ON DELETE CASCADE,
    skill_name       TEXT,
    skill_path       TEXT,
    invoked_unix_ns  INTEGER,
    conversation_id  TEXT,
    agent_run_pk     INTEGER REFERENCES agent_runs(agent_run_pk) ON DELETE SET NULL,
    chat_turn_pk     INTEGER REFERENCES chat_turns(turn_pk) ON DELETE SET NULL,
    UNIQUE(span_pk, invoked_unix_ns, skill_name)
);
CREATE INDEX IF NOT EXISTS idx_skill_name ON skill_invocations(skill_name);
CREATE INDEX IF NOT EXISTS idx_skill_conv ON skill_invocations(conversation_id);
CREATE INDEX IF NOT EXISTS idx_skill_agent_run ON skill_invocations(agent_run_pk);
CREATE INDEX IF NOT EXISTS idx_skill_chat_turn ON skill_invocations(chat_turn_pk);

CREATE TABLE IF NOT EXISTS context_snapshots (
    ctx_pk           INTEGER PRIMARY KEY AUTOINCREMENT,
    span_pk          INTEGER REFERENCES spans(span_pk) ON DELETE SET NULL,
    chat_turn_pk     INTEGER REFERENCES chat_turns(turn_pk) ON DELETE SET NULL,
    conversation_id  TEXT,
    captured_ns      INTEGER NOT NULL,
    token_limit      INTEGER,
    current_tokens   INTEGER,
    messages_length  INTEGER,
    input_tokens     INTEGER,
    output_tokens    INTEGER,
    cache_read_tokens INTEGER,
    reasoning_tokens INTEGER,
    source           TEXT,
    UNIQUE(span_pk, source, captured_ns)
);
CREATE INDEX IF NOT EXISTS idx_ctx_conv_time ON context_snapshots(conversation_id, captured_ns DESC);
CREATE INDEX IF NOT EXISTS idx_ctx_chat_turn ON context_snapshots(chat_turn_pk);
