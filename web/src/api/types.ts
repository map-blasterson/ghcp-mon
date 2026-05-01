// Span-canonical TypeScript types matching src/api/mod.rs.

export type Nullable<T> = T | null;
export type UnixNs = number;
export type Iso8601 = string;

export interface HealthzResponse { ok: boolean; }

// ------------------------- sessions ------------------------------------

export interface SessionSummary {
  conversation_id: string;
  first_seen_ns: Nullable<UnixNs>;
  last_seen_ns: Nullable<UnixNs>;
  latest_model: Nullable<string>;
  chat_turn_count: number;
  tool_call_count: number;
  agent_run_count: number;
  // Sidecar metadata read from
  // ~/.copilot/session-state/<cid>/workspace.yaml. Optional because
  // the dir may not exist (older sessions, different host, etc.).
  local_name?: Nullable<string>;
  user_named?: Nullable<boolean>;
  cwd?: Nullable<string>;
  branch?: Nullable<string>;
}
export interface ListSessionsResponse { sessions: SessionSummary[]; }

export interface SessionDetail extends SessionSummary {
  span_count: number;
}

// ------------------------- spans ---------------------------------------

export type KindClass =
  | "invoke_agent"
  | "chat"
  | "execute_tool"
  | "external_tool"
  | "other";

export interface SpanRow {
  span_pk: number;
  trace_id: string;
  span_id: string;
  parent_span_id: Nullable<string>;
  name: string;
  kind_class: KindClass;
  start_unix_ns: Nullable<UnixNs>;
  end_unix_ns: Nullable<UnixNs>;
  ingestion_state: string; // "real" | "placeholder"
}
export interface ListSpansResponse { spans: SpanRow[]; }

// Projection sub-blocks (all optional; an "other" span has none).
export interface ChatTurnProjection {
  turn_pk: number;
  conversation_id: Nullable<string>;
  agent_run_pk: Nullable<number>;
  interaction_id: Nullable<string>;
  turn_id: Nullable<string>;
  model: Nullable<string>;
  input_tokens: Nullable<number>;
  output_tokens: Nullable<number>;
  cache_read_tokens: Nullable<number>;
  reasoning_tokens: Nullable<number>;
  tool_call_count: number;
}
export interface ToolCallProjection {
  tool_call_pk: number;
  call_id: Nullable<string>;
  tool_name: Nullable<string>;
  tool_type: Nullable<string>;
  conversation_id: Nullable<string>;
  agent_run_pk: Nullable<number>;
  status_code: Nullable<number>;
}
export interface AgentRunProjection {
  agent_run_pk: number;
  conversation_id: Nullable<string>;
  agent_id: Nullable<string>;
  agent_name: Nullable<string>;
  agent_version: Nullable<string>;
  parent_agent_run_pk: Nullable<number>;
  parent_span_pk: Nullable<number>;
}
export interface ExternalToolCallProjection {
  ext_pk: number;
  call_id: Nullable<string>;
  tool_name: Nullable<string>;
  paired_tool_call_pk: Nullable<number>;
  conversation_id: Nullable<string>;
  agent_run_pk: Nullable<number>;
}

export interface SpanProjection {
  chat_turn?: ChatTurnProjection;
  tool_call?: ToolCallProjection;
  agent_run?: AgentRunProjection;
  external_tool_call?: ExternalToolCallProjection;
}

export interface SpanRef {
  span_pk: number;
  trace_id: string;
  span_id: string;
  name: string;
  kind_class: KindClass;
}

export interface SpanEvent {
  event_pk: number;
  name: string;
  time_unix_ns: UnixNs;
  attributes: Record<string, unknown> | null;
}

export interface SpanFull {
  span_pk: number;
  trace_id: string;
  span_id: string;
  parent_span_id: Nullable<string>;
  name: string;
  kind: string;
  kind_class: KindClass;
  start_unix_ns: Nullable<UnixNs>;
  end_unix_ns: Nullable<UnixNs>;
  duration_ns: Nullable<number>;
  status_message: Nullable<string>;
  ingestion_state: string;
  scope_name: Nullable<string>;
  scope_version: Nullable<string>;
  attributes: Record<string, unknown> | null;
  resource: Record<string, unknown> | null;
}

export interface SpanDetail {
  span: SpanFull;
  events: SpanEvent[];
  parent: Nullable<SpanRef>;
  children: SpanRef[];
  projection: SpanProjection;
}

// ------------------------- span tree -----------------------------------

export interface SpanNode {
  span_pk: number;
  trace_id: string;
  span_id: string;
  parent_span_id: Nullable<string>;
  name: string;
  kind_class: KindClass;
  ingestion_state: string;
  start_unix_ns: Nullable<UnixNs>;
  end_unix_ns: Nullable<UnixNs>;
  projection: SpanProjection;
  children: SpanNode[];
}

export interface SessionSpanTreeResponse {
  conversation_id: string;
  tree: SpanNode[];
}

// ------------------------- traces (live grouping) ----------------------

export interface KindCounts {
  chat: number;
  execute_tool: number;
  external_tool: number;
  invoke_agent: number;
  other: number;
}

export interface TraceRootRef {
  span_pk: number;
  trace_id: string;
  span_id: string;
  parent_span_id: Nullable<string>;
  name: string;
  kind_class: KindClass;
  ingestion_state: string;
}

export interface TraceSummary {
  trace_id: string;
  first_seen_ns: Nullable<UnixNs>;
  last_seen_ns: Nullable<UnixNs>;
  span_count: number;
  placeholder_count: number;
  kind_counts: KindCounts;
  root: Nullable<TraceRootRef>;
  conversation_id: Nullable<string>;
}
export interface ListTracesResponse { traces: TraceSummary[]; }

export interface TraceDetailResponse {
  trace_id: string;
  conversation_id: Nullable<string>;
  tree: SpanNode[];
}

// ------------------------- session-scoped projections -------------------

export interface ContextSnapshot {
  ctx_pk: number;
  span_pk: Nullable<number>;
  captured_ns: UnixNs;
  token_limit: Nullable<number>;
  current_tokens: Nullable<number>;
  messages_length: Nullable<number>;
  input_tokens: Nullable<number>;
  output_tokens: Nullable<number>;
  cache_read_tokens: Nullable<number>;
  reasoning_tokens: Nullable<number>;
  source: Nullable<string>;
}
export interface ListSessionContextsResponse {
  conversation_id: string;
  context_snapshots: ContextSnapshot[];
}

export interface ToolDefinition {
  type?: string;
  name?: string;
  [k: string]: unknown;
}

// ------------------------- raw -----------------------------------------

export type RawRecordType =
  | "span" | "metric" | "log"
  | "otlp-traces" | "otlp-metrics" | "otlp-logs"
  | "envelope-batch";
export interface RawRecord {
  id: number;
  received_at: Iso8601;
  source: string;
  record_type: RawRecordType;
  content_type: Nullable<string>;
  body: unknown;
}
export interface ListRawResponse { raw: RawRecord[]; }

// ------------------------- WS envelopes ---------------------------------

export type WsKind = "hello" | "span" | "metric" | "log" | "derived" | "trace";
export type WsEntity =
  | "control"
  | "span"
  | "placeholder"
  | "metric"
  | "log"
  | "session"
  | "chat_turn"
  | "tool_call"
  | "external_tool_call"
  | "agent_run"
  | "context_snapshot"
  | "trace";

export interface WsEnvelope<P = unknown> {
  kind: WsKind;
  entity: WsEntity;
  payload: P;
}

export interface SpanInsertedPayload {
  action: "insert" | "upgrade";
  trace_id: string;
  span_id: string;
  parent_span_id: Nullable<string>;
  name: string;
  kind_class: KindClass;
  ingestion_state: string;
  span_pk: number;
}
export interface PlaceholderPayload {
  action: "insert";
  trace_id: string;
  span_id: string;
  span_pk: number;
}
export interface MetricInsertedPayload { name: string; points: number; }
export interface SessionUpsertedPayload {
  action: "update";
  conversation_id: string;
  latest_model: Nullable<string>;
}
export interface ChatTurnUpsertedPayload {
  action: "upsert";
  turn_pk: number;
  span_pk: number;
  conversation_id: Nullable<string>;
  agent_run_pk: Nullable<number>;
  interaction_id: Nullable<string>;
  turn_id: Nullable<string>;
}
export interface ToolCallUpsertedPayload {
  action: "upsert";
  tool_call_pk: number;
  span_pk: number;
  tool_name: Nullable<string>;
  call_id: Nullable<string>;
  agent_run_pk: Nullable<number>;
  conversation_id: Nullable<string>;
}
export interface AgentRunUpsertedPayload {
  action: "upsert";
  agent_run_pk: number;
  span_pk: number;
  conversation_id: Nullable<string>;
  agent_name: Nullable<string>;
  parent_agent_run_pk: Nullable<number>;
  parent_span_pk: Nullable<number>;
}
