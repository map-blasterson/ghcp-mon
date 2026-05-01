import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect } from "react";
import type React from "react";
import { api } from "../api/client";
import type { Column } from "../state/workspace";
import { useWorkspace } from "../state/workspace";
import { ColumnHeader } from "../components/ColumnHeader";
import { useLiveFeed } from "../state/live";
import { fmtRelative } from "../components/content";

export function LiveSessionsScenario({ column }: { column: Column }) {
  const qc = useQueryClient();
  const q = useQuery({
    queryKey: ["sessions"],
    queryFn: () => api.listSessions({ limit: 50 }),
  });
  const { tick } = useLiveFeed([
    { kind: "derived", entity: "session" },
    { kind: "derived", entity: "chat_turn" },
  ]);
  useEffect(() => {
    qc.invalidateQueries({ queryKey: ["sessions"] });
  }, [tick, qc]);

  const updateColumn = useWorkspace((s) => s.updateColumn);
  const columns = useWorkspace((s) => s.columns);

  const onSelect = (id: string) => {
    columns.forEach((c) => {
      if (c.id === column.id) return;
      if (
        [
          "spans",
          "input_breakdown",
          "file_touches",
        ].includes(c.scenarioType)
      ) {
        updateColumn(c.id, { config: { ...c.config, session: id } });
      }
    });
    updateColumn(column.id, { config: { ...column.config, session: id } });
  };

  const onDelete = async (id: string, e: React.MouseEvent) => {
    e.stopPropagation();
    if (!confirm(`Delete session ${id.slice(0, 8)}? This removes all spans, turns, and tool calls in its trace(s).`)) {
      return;
    }
    try {
      await api.deleteSession(id);
    } catch (err) {
      alert(`Failed to delete session: ${String(err)}`);
      return;
    }
    columns.forEach((c) => {
      if (c.config.session === id) {
        updateColumn(c.id, { config: { ...c.config, session: undefined } });
      }
    });
    qc.invalidateQueries({ queryKey: ["sessions"] });
  };

  return (
    <>
      <ColumnHeader column={column}>
        <span className="dim">{q.data?.sessions.length ?? 0} sessions</span>
        <button onClick={() => qc.invalidateQueries({ queryKey: ["sessions"] })}>↻</button>
      </ColumnHeader>
      <div className="col-body">
        <div className="list">
          {q.isLoading && <div className="empty-state">loading…</div>}
          {q.error && <div className="empty-state">error: {String(q.error)}</div>}
          {q.data?.sessions.map((s) => {
            const shortId = s.conversation_id.slice(0, 8);
            const name = s.local_name && s.local_name.trim().length > 0 ? s.local_name : null;
            return (
              <div
                key={s.conversation_id}
                className={`row session-row${column.config.session === s.conversation_id ? " sel" : ""}`}
                onClick={() => onSelect(s.conversation_id)}
              >
                <div className="session-row-top">
                  <span className="session-name" title={name ?? s.conversation_id}>
                    {name ?? <span className="mono">{shortId}</span>}
                  </span>
                  {name && <span className="session-id mono dim">{shortId}</span>}
                  {s.user_named === false && name && (
                    <span className="session-tag dim" title="auto-summarized name (use /rename in copilot to set)">auto</span>
                  )}
                  <span className="right">{fmtRelative(s.last_seen_ns)}</span>
                  <button
                    className="row-action"
                    title="Delete session"
                    aria-label={`Delete session ${s.conversation_id}`}
                    onClick={(e) => onDelete(s.conversation_id, e)}
                  >
                    ✕
                  </button>
                </div>
                <div className="session-row-bot">
                  <span className="sec">{s.latest_model ?? "—"}</span>
                  <span className="sep">·</span>
                  <span className="sec">{s.chat_turn_count} turn{s.chat_turn_count === 1 ? "" : "s"}</span>
                  <span className="sep">·</span>
                  <span className="sec">{s.tool_call_count} tool{s.tool_call_count === 1 ? "" : "s"}</span>
                  <span className="sep">·</span>
                  <span className="sec">{s.agent_run_count} agent{s.agent_run_count === 1 ? "" : "s"}</span>
                  {s.branch && (
                    <>
                      <span className="sep">·</span>
                      <span className="sec mono" title={s.cwd ?? undefined}>{s.branch}</span>
                    </>
                  )}
                </div>
              </div>
            );
          })}
          {q.data && q.data.sessions.length === 0 && (
            <div className="empty-state">no sessions yet — replay or send OTLP traffic to :4318</div>
          )}
        </div>
      </div>
    </>
  );
}
