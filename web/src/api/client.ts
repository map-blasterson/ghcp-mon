import type {
  ListRawResponse,
  ListSessionContextsResponse,
  ListSessionsResponse,
  ListSpansResponse,
  ListTracesResponse,
  RawRecordType,
  SearchResponse,
  SessionDetail,
  SessionSpanTreeResponse,
  SpanDetail,
  KindClass,
  TraceDetailResponse,
} from "./types";

export const API_BASE = "http://127.0.0.1:4319";
export const WS_URL = "ws://127.0.0.1:4319/ws/events";

async function getJson<T>(path: string): Promise<T> {
  const r = await fetch(`${API_BASE}${path}`);
  if (!r.ok) throw new Error(`${r.status} ${r.statusText} for ${path}`);
  return (await r.json()) as T;
}

async function deleteJson<T>(path: string): Promise<T> {
  const r = await fetch(`${API_BASE}${path}`, { method: "DELETE" });
  if (!r.ok) throw new Error(`${r.status} ${r.statusText} for ${path}`);
  return (await r.json()) as T;
}

function qs(params: Record<string, string | number | undefined | null>): string {
  const q = Object.entries(params)
    .filter(([, v]) => v !== undefined && v !== null && v !== "")
    .map(([k, v]) => `${encodeURIComponent(k)}=${encodeURIComponent(String(v))}`)
    .join("&");
  return q ? `?${q}` : "";
}

export interface ListSpansOpts {
  session?: string;
  kind?: KindClass;
  since?: number;
  limit?: number;
}

export const api = {
  listSessions: (opts: { limit?: number; since?: number } = {}) =>
    getJson<ListSessionsResponse>(`/api/sessions${qs({ limit: opts.limit ?? 50, since: opts.since })}`),
  getSession: (cid: string) =>
    getJson<SessionDetail>(`/api/sessions/${cid}`),
  deleteSession: (cid: string) =>
    deleteJson<{ deleted: boolean; conversation_id: string; trace_count: number }>(
      `/api/sessions/${cid}`,
    ),
  getSessionSpanTree: (cid: string) =>
    getJson<SessionSpanTreeResponse>(`/api/sessions/${cid}/span-tree`),
  listSessionContexts: (cid: string) =>
    getJson<ListSessionContextsResponse>(`/api/sessions/${cid}/contexts`),
  listSpans: (opts: ListSpansOpts = {}) =>
    getJson<ListSpansResponse>(
      `/api/spans${qs({ session: opts.session, kind: opts.kind, since: opts.since, limit: opts.limit ?? 100 })}`
    ),
  getSpan: (trace_id: string, span_id: string) =>
    getJson<SpanDetail>(`/api/spans/${trace_id}/${span_id}`),
  listTraces: (opts: { limit?: number; since?: number } = {}) =>
    getJson<ListTracesResponse>(`/api/traces${qs({ limit: opts.limit ?? 50, since: opts.since })}`),
  getTrace: (trace_id: string) =>
    getJson<TraceDetailResponse>(`/api/traces/${trace_id}`),
  listRaw: (opts: { type?: RawRecordType; limit?: number } = {}) =>
    getJson<ListRawResponse>(`/api/raw${qs({ type: opts.type, limit: opts.limit ?? 100 })}`),
  searchSpans: (opts: { q: string; session: string; limit?: number; mode?: string }) =>
    getJson<SearchResponse>(`/api/search${qs({ q: opts.q, session: opts.session, limit: opts.limit, mode: opts.mode })}`),
};
