import { create } from "zustand";
import { persist } from "zustand/middleware";
import type { KindClass } from "../api/types";

export type ScenarioType =
  | "live_sessions"
  | "spans"
  | "tool_detail"
  | "raw_browser"
  | "input_breakdown"
  | "file_touches";

export interface ColumnConfig {
  // Selection is (trace_id, span_id) — the canonical span key.
  session?: string;
  selected_trace_id?: string;
  selected_span_id?: string;
  raw_type?: string;
  tool_name_filter?: string;
  kind_filter?: KindClass;
}

export interface Column {
  id: string;
  scenarioType: ScenarioType;
  title: string;
  config: ColumnConfig;
  width: number; // grid fr units
}

interface WorkspaceState {
  columns: Column[];
  contextWidgetHeightVh: number;
  contextWidgetVisible: boolean;
  setColumns: (cs: Column[]) => void;
  addColumn: (c: Column) => void;
  removeColumn: (id: string) => void;
  updateColumn: (id: string, patch: Partial<Column>) => void;
  moveColumn: (id: string, dir: -1 | 1) => void;
  setContextWidgetHeight: (vh: number) => void;
  setContextWidgetVisible: (v: boolean) => void;
  resetDefault: () => void;
}

let nextId = 1;
export function genId(prefix = "col"): string {
  return `${prefix}-${Date.now().toString(36)}-${(nextId++).toString(36)}`;
}

const defaultColumns = (): Column[] => [
  { id: genId(), scenarioType: "live_sessions", title: "Sessions", config: {}, width: 1 },
  { id: genId(), scenarioType: "spans", title: "Traces", config: {}, width: 1.4 },
  { id: genId(), scenarioType: "tool_detail", title: "Tool detail", config: {}, width: 1.4 },
  { id: genId(), scenarioType: "input_breakdown", title: "Chat detail", config: {}, width: 1.6 },
];

export const useWorkspace = create<WorkspaceState>()(
  persist(
    (set) => ({
      columns: defaultColumns(),
      contextWidgetHeightVh: 15,
      contextWidgetVisible: true,
      setColumns: (cs) => set({ columns: cs }),
      addColumn: (c) => set((s) => ({ columns: [...s.columns, c] })),
      removeColumn: (id) =>
        set((s) => ({ columns: s.columns.filter((c) => c.id !== id) })),
      updateColumn: (id, patch) =>
        set((s) => ({
          columns: s.columns.map((c) => (c.id === id ? { ...c, ...patch, config: { ...c.config, ...(patch.config ?? {}) } } : c)),
        })),
      moveColumn: (id, dir) =>
        set((s) => {
          const i = s.columns.findIndex((c) => c.id === id);
          if (i < 0) return s;
          const j = i + dir;
          if (j < 0 || j >= s.columns.length) return s;
          const cs = [...s.columns];
          [cs[i], cs[j]] = [cs[j], cs[i]];
          return { columns: cs };
        }),
      setContextWidgetHeight: (vh) => set({ contextWidgetHeightVh: vh }),
      setContextWidgetVisible: (v) => set({ contextWidgetVisible: v }),
      resetDefault: () => set({ columns: defaultColumns(), contextWidgetHeightVh: 15, contextWidgetVisible: true }),
    }),
    {
      name: "ghcp-mon-workspace-v6",
      migrate: (persisted: any) => {
        if (persisted && Array.isArray(persisted.columns)) {
          const dropped = new Set(["context_growth", "tool_registry", "context_inspector", "shell_io"]);
          persisted.columns = persisted.columns.filter((c: any) => !dropped.has(c.scenarioType));
        }
        return persisted;
      },
    }
  )
);

export const SCENARIO_LABELS: Record<ScenarioType, string> = {
  live_sessions: "Live sessions",
  spans: "Spans",
  tool_detail: "Tool detail inspector",
  raw_browser: "Raw OTel record browser",
  input_breakdown: "Chat detail",
  file_touches: "File touches",
};
