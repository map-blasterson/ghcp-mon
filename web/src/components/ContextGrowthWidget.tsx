import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect, useMemo, useRef } from "react";
import { api } from "../api/client";
import type { SpanNode } from "../api/types";
import { useLiveFeed } from "../state/live";
import { useHoverState } from "../state/hover";
import { useWorkspace } from "../state/workspace";

export function ContextGrowthWidget() {
  const qc = useQueryClient();
  const columns = useWorkspace((s) => s.columns);
  const heightVh = useWorkspace((s) => s.contextWidgetHeightVh);
  const visible = useWorkspace((s) => s.contextWidgetVisible);
  const setHeight = useWorkspace((s) => s.setContextWidgetHeight);
  const setVisible = useWorkspace((s) => s.setContextWidgetVisible);

  // Pick the first session referenced by any column.
  const session = useMemo(() => {
    for (const c of columns) {
      if (c.config.session) return c.config.session;
    }
    return "";
  }, [columns]);

  const ctxQ = useQuery({
    queryKey: ["session-contexts", session],
    queryFn: () => api.listSessionContexts(session),
    enabled: !!session,
  });
  const treeQ = useQuery({
    queryKey: ["session-span-tree", session],
    queryFn: () => api.getSessionSpanTree(session),
    enabled: !!session,
  });
  const { tick } = useLiveFeed([
    { kind: "derived", entity: "chat_turn" },
    { kind: "span", entity: "span" },
    { kind: "span", entity: "placeholder" },
  ]);
  useEffect(() => {
    if (!session) return;
    qc.invalidateQueries({ queryKey: ["session-contexts", session] });
    qc.invalidateQueries({ queryKey: ["session-span-tree", session] });
  }, [tick, session, qc]);

  // Top-level chat span_pks: chat spans whose ancestor chain contains
  // at most one `invoke_agent` ancestor (the root). Sub-agent chats —
  // those nested under an `invoke_agent task` (or any nested
  // `invoke_agent`) — have two or more, and are excluded so their
  // context tokens don't get charted alongside the parent agent's.
  const topLevelChatPks = useMemo(() => {
    const pks = new Set<number>();
    const walk = (nodes: SpanNode[], invokeDepth: number) => {
      for (const n of nodes) {
        const isChat = n.kind_class === "chat";
        const isInvoke = n.kind_class === "invoke_agent";
        if (isChat && invokeDepth <= 1 && n.span_pk != null) pks.add(n.span_pk);
        walk(n.children ?? [], invokeDepth + (isInvoke ? 1 : 0));
      }
    };
    walk(treeQ.data?.tree ?? [], 0);
    return pks;
  }, [treeQ.data]);

  const snaps = useMemo(
    () => (ctxQ.data?.context_snapshots ?? []).slice().sort((a, b) => a.captured_ns - b.captured_ns),
    [ctxQ.data]
  );

  const turnRows = useMemo(() => {
    // Group by span_pk so chat_span and usage_info_event rows for the
    // same chat merge. Drop snapshots belonging to sub-agent chats once
    // the tree is loaded.
    const treeKnown = topLevelChatPks.size > 0;
    const bySpan = new Map<number, typeof snaps>();
    for (const s of snaps) {
      if (s.span_pk == null) continue;
      if (treeKnown && !topLevelChatPks.has(s.span_pk)) continue;
      const arr = bySpan.get(s.span_pk) ?? [];
      arr.push(s);
      bySpan.set(s.span_pk, arr);
    }
    const pickIn = <K extends keyof typeof snaps[number]>(ss: typeof snaps, k: K) => {
      for (let i = ss.length - 1; i >= 0; i--) {
        const v = ss[i][k];
        if (v !== null && v !== undefined) return v as number;
      }
      return null;
    };
    return [...bySpan.entries()]
      .map(([span_pk, ss]) => ({
        turn_pk: span_pk,
        first_seen_ns: ss[0].captured_ns,
        input: (pickIn(ss, "input_tokens") as number | null) ?? 0,
        output: (pickIn(ss, "output_tokens") as number | null) ?? 0,
        reasoning: (pickIn(ss, "reasoning_tokens") as number | null) ?? 0,
        token_limit: pickIn(ss, "token_limit") as number | null,
        current_tokens: pickIn(ss, "current_tokens") as number | null,
      }))
      .sort((a, b) => a.first_seen_ns - b.first_seen_ns);
  }, [snaps, topLevelChatPks]);

  const maxTokenLimit = turnRows.reduce((m, r) => Math.max(m, r.token_limit ?? 0), 0);
  const maxCurrent = turnRows.reduce((m, r) => Math.max(m, r.current_tokens ?? 0), 0);
  const yMax = Math.max(maxTokenLimit * 1.05, maxCurrent);

  const dragRef = useRef<{ startY: number; startVh: number } | null>(null);
  const onResizePointerDown = (e: React.PointerEvent) => {
    e.preventDefault();
    const target = e.currentTarget as HTMLElement;
    target.setPointerCapture(e.pointerId);
    dragRef.current = { startY: e.clientY, startVh: heightVh };
    const onMove = (ev: PointerEvent) => {
      if (!dragRef.current) return;
      const dy = ev.clientY - dragRef.current.startY;
      const dvh = (dy / window.innerHeight) * 100;
      const next = Math.max(5, Math.min(80, dragRef.current.startVh - dvh));
      setHeight(next);
    };
    const onUp = () => {
      target.releasePointerCapture(e.pointerId);
      dragRef.current = null;
      window.removeEventListener("pointermove", onMove);
      window.removeEventListener("pointerup", onUp);
    };
    window.addEventListener("pointermove", onMove);
    window.addEventListener("pointerup", onUp);
  };

  if (!visible) {
    return (
      <div className="ctx-widget collapsed">
        <button onClick={() => setVisible(true)} title="Show context growth">▾ context growth</button>
      </div>
    );
  }

  return (
    <div className="ctx-widget" style={{ height: `${heightVh}vh` }}>
      <div className="ctx-widget-header">
        <span className="brand-mini">context growth</span>
        <span className="dim">session</span>
        <span className="mono">{session ? session.slice(0, 8) : "—"}</span>
        <span className="dim">{turnRows.length} turn{turnRows.length === 1 ? "" : "s"}</span>
        <span className="spacer" />
        <button onClick={() => setVisible(false)} title="Hide widget">×</button>
      </div>
      <div className="ctx-widget-body">
        {!session && <div className="empty-state">pick a session</div>}
        {session && turnRows.length === 0 && <div className="empty-state">no context snapshots</div>}
        {session && turnRows.length > 0 && (
          <CtxStackChart rows={turnRows} max={yMax} tokenLimit={maxTokenLimit} />
        )}
      </div>
      <div
        className="ctx-widget-resizer"
        onPointerDown={onResizePointerDown}
        title="Drag to resize"
      />
    </div>
  );
}

interface ChartRow {
  turn_pk: number;
  input: number;
  output: number;
  reasoning: number;
  token_limit: number | null;
  current_tokens: number | null;
}

const STACK_COLORS = {
  input: "var(--accent)",
  output: "#d4a017",
  reasoning: "#b65cff",
};

function fmtTokens(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`;
  if (n >= 1_000) return `${(n / 1_000).toFixed(1)}k`;
  return `${n}`;
}

function CtxStackChart({ rows, max, tokenLimit }: { rows: ChartRow[]; max: number; tokenLimit: number }) {
  if (max <= 0) return <div className="empty-state">no token data</div>;
  const limitPct = tokenLimit > 0 ? (tokenLimit / max) * 100 : null;
  const hoveredChatPk = useHoverState((s) => s.hoveredChatPk);
  return (
    <div className="ctx-stack-chart">
      <div className="ctx-stack-bars" style={{ position: "relative" }}>
        {rows.map((r) => {
          const total = r.input + r.output + r.reasoning;
          const h = (v: number) => `${(v / max) * 100}%`;
          const tip = `chat_turn #${r.turn_pk}\ntotal ${fmtTokens(total)}\ninput ${fmtTokens(r.input)} • output ${fmtTokens(r.output)} • reasoning ${fmtTokens(r.reasoning)}\nlimit ${r.token_limit != null ? fmtTokens(r.token_limit) : "—"} · current ${r.current_tokens != null ? fmtTokens(r.current_tokens) : "—"}`;
          const isHovered = hoveredChatPk === r.turn_pk;
          return (
            <div
              className={`ctx-stack-col${isHovered ? " hovered" : ""}`}
              key={r.turn_pk}
              title={tip}
            >
              <div className="ctx-stack" style={{ height: `${(total / max) * 100}%` }}>
                <span style={{ height: h(r.input), background: STACK_COLORS.input }} />
                <span style={{ height: h(r.output), background: STACK_COLORS.output }} />
                <span style={{ height: h(r.reasoning), background: STACK_COLORS.reasoning }} />
              </div>
              {isHovered && <div className="ctx-stack-hover-bar" />}
            </div>
          );
        })}
        {limitPct != null && (
          <div
            className="ctx-stack-limit-line"
            style={{ bottom: `calc(14px + (100% - 14px) * ${limitPct} / 100)` }}
            title={`token limit ${fmtTokens(tokenLimit)}`}
          />
        )}
      </div>
      <div className="ctx-stack-legend">
        <span><i style={{ background: STACK_COLORS.input }} />input</span>
        <span><i style={{ background: STACK_COLORS.output }} />output</span>
        <span><i style={{ background: STACK_COLORS.reasoning }} />reasoning</span>
        <span><i style={{ background: "#ffd54a" }} />limit {fmtTokens(tokenLimit)}</span>
        <span className="right dim">y-max {fmtTokens(max)} tok</span>
      </div>
    </div>
  );
}
