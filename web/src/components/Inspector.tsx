import { useQuery } from "@tanstack/react-query";
import { api } from "../api/client";
import { JsonView } from "./JsonView";
import { fmtClock, fmtNs } from "./content";
import { KindBadge, RollingDots } from "./KindBadge";
import type { SpanDetail } from "../api/types";

export function SpanInspector({
  trace_id,
  span_id,
}: {
  trace_id: string;
  span_id: string;
}) {
  const q = useQuery({
    queryKey: ["span", trace_id, span_id],
    queryFn: () => api.getSpan(trace_id, span_id),
  });
  if (q.isLoading) return <div className="empty-state">loading…</div>;
  if (q.error || !q.data) return <div className="empty-state">span not found</div>;
  return <SpanDetailView detail={q.data} />;
}

export function SpanDetailView({ detail }: { detail: SpanDetail }) {
  const { span, events, parent, children, projection } = detail;
  const dur =
    span.duration_ns ??
    (span.start_unix_ns != null && span.end_unix_ns != null
      ? span.end_unix_ns - span.start_unix_ns
      : null);
  return (
    <div className="col-body" style={{ overflow: "auto" }}>
      <div className="section">
        <h4>{span.name}</h4>
        <div className="kv">
          <span className="k">kind</span>
          <span className="v"><KindBadge k={span.kind_class} /> <span className="dim">({span.kind})</span></span>
          <span className="k">ingestion</span>
          <span className="v">
            {span.ingestion_state}
            {span.ingestion_state === "placeholder" && (
              <span className="tag warn" style={{ marginLeft: 6 }}><RollingDots /></span>
            )}
          </span>
          <span className="k">trace_id</span>
          <span className="v mono">{span.trace_id}</span>
          <span className="k">span_id</span>
          <span className="v mono">{span.span_id}</span>
          <span className="k">start</span>
          <span className="v">{fmtClock(span.start_unix_ns)}</span>
          <span className="k">duration</span>
          <span className="v">{fmtNs(dur)}</span>
          {span.status_message && (
            <>
              <span className="k">status</span>
              <span className="v">{span.status_message}</span>
            </>
          )}
          <span className="k">scope</span>
          <span className="v">
            {span.scope_name ?? "—"}
            {span.scope_version ? ` ${span.scope_version}` : ""}
          </span>
        </div>
      </div>

      {Object.keys(projection ?? {}).length > 0 && (
        <div className="section">
          <h4>projection</h4>
          {projection.chat_turn && (
            <details open>
              <summary>chat_turn</summary>
              <JsonView value={projection.chat_turn} />
            </details>
          )}
          {projection.tool_call && (
            <details open>
              <summary>tool_call</summary>
              <JsonView value={projection.tool_call} />
            </details>
          )}
          {projection.agent_run && (
            <details open>
              <summary>agent_run</summary>
              <JsonView value={projection.agent_run} />
            </details>
          )}
          {projection.external_tool_call && (
            <details open>
              <summary>external_tool_call</summary>
              <JsonView value={projection.external_tool_call} />
            </details>
          )}
        </div>
      )}

      <div className="section">
        <h4>relations</h4>
        <div className="kv">
          <span className="k">parent</span>
          <span className="v mono">
            {parent ? `${parent.name} (${parent.span_id.slice(0, 8)})` : "—"}
          </span>
          <span className="k">children</span>
          <span className="v">{children.length}</span>
        </div>
        {children.length > 0 && (
          <div className="list">
            {children.map((c) => (
              <div key={c.span_pk} className="row">
                <span className="pri mono">{c.name}</span>
                <KindBadge k={c.kind_class} />
                <span className="right dim mono">{c.span_id.slice(0, 8)}</span>
              </div>
            ))}
          </div>
        )}
      </div>

      <div className="section">
        <h4>events ({events.length})</h4>
        {events.length === 0 ? (
          <div className="dim">none</div>
        ) : (
          <JsonView value={events} collapsed />
        )}
      </div>

      <div className="section">
        <h4>attributes</h4>
        <JsonView value={span.attributes} collapsed />
      </div>

      {span.resource && (
        <div className="section">
          <h4>resource</h4>
          <JsonView value={span.resource} collapsed />
        </div>
      )}
    </div>
  );
}
