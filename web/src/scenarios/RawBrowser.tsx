import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect, useState } from "react";
import { api } from "../api/client";
import type { Column } from "../state/workspace";
import { useWorkspace } from "../state/workspace";
import { ColumnHeader } from "../components/ColumnHeader";
import { useLiveFeed } from "../state/live";
import { JsonView } from "../components/JsonView";
import type { RawRecordType } from "../api/types";

const TYPES: Array<RawRecordType | ""> = [
  "", "span", "metric", "log", "otlp-traces", "otlp-metrics", "otlp-logs", "envelope-batch",
];

export function RawBrowserScenario({ column }: { column: Column }) {
  const qc = useQueryClient();
  const updateColumn = useWorkspace((s) => s.updateColumn);
  const t = column.config.raw_type as RawRecordType | undefined;
  const [sel, setSel] = useState<number | null>(null);

  const q = useQuery({
    queryKey: ["raw", t],
    queryFn: () => api.listRaw({ type: t || undefined, limit: 200 }),
  });
  const { tick } = useLiveFeed([
    { kind: "span", entity: "span" },
    { kind: "metric", entity: "metric" },
  ]);
  useEffect(() => {
    qc.invalidateQueries({ queryKey: ["raw", t] });
  }, [tick, t, qc]);

  const selected = q.data?.raw.find((r) => r.id === sel);

  return (
    <>
      <ColumnHeader column={column}>
        <span className="dim">type</span>
        <select
          value={t ?? ""}
          onChange={(e) => updateColumn(column.id, { config: { ...column.config, raw_type: e.target.value || undefined } })}
        >
          {TYPES.map((x) => <option key={x} value={x}>{x || "any"}</option>)}
        </select>
        <span className="dim">{q.data?.raw.length ?? 0}</span>
      </ColumnHeader>
      <div className="col-body" style={{ display: "grid", gridTemplateRows: "1fr 1fr", overflow: "hidden" }}>
        <div className="list" style={{ overflow: "auto", borderBottom: "1px solid var(--border)" }}>
          {q.data?.raw.map((r) => (
            <div key={r.id} className={`row${sel === r.id ? " sel" : ""}`} onClick={() => setSel(r.id)}>
              <span className="pri">{r.record_type}</span>
              <span className="sec mono">#{r.id}</span>
              <span className="right dim">{r.received_at}</span>
            </div>
          ))}
        </div>
        <div style={{ overflow: "auto" }}>
          {selected ? <JsonView value={selected.body} /> : <div className="empty-state">select a record</div>}
        </div>
      </div>
    </>
  );
}
