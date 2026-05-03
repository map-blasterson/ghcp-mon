import { useEffect, useMemo, useRef } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { api } from "../api/client";
import type { SpanNode } from "../api/types";
import { useWorkspace } from "../state/workspace";
import { useHoverState } from "../state/hover";
import { useLiveFeed } from "../state/live";

interface ChatSpanInfo {
  span_pk: number;
  start_unix_ns: number | null;
  invokeAgentDepth: number;
}

function collectChatSpans(
  nodes: SpanNode[],
  depth: number,
  out: ChatSpanInfo[],
): void {
  for (const n of nodes) {
    if (n.kind_class === "chat") {
      out.push({
        span_pk: n.span_pk,
        start_unix_ns: n.start_unix_ns,
        invokeAgentDepth: depth,
      });
    }
    const childDepth = depth + (n.kind_class === "invoke_agent" ? 1 : 0);
    collectChatSpans(n.children, childDepth, out);
  }
}

export function ContextGrowthWidget() {
  const visible = useWorkspace((s) => s.contextWidgetVisible);
  const setVisible = useWorkspace((s) => s.setContextWidgetVisible);
  const heightVh = useWorkspace((s) => s.contextWidgetHeightVh);
  const setHeightVh = useWorkspace((s) => s.setContextWidgetHeight);
  const columns = useWorkspace((s) => s.columns);
  const hoveredChatPk = useHoverState((s) => s.hoveredChatPk);
  const qc = useQueryClient();

  // First column with a session bound is the widget's session.
  const session = useMemo(() => {
    for (const c of columns) {
      if (c.config.session) return c.config.session;
    }
    return undefined;
  }, [columns]);

  const contextsQ = useQuery({
    queryKey: ["session-contexts", session],
    queryFn: () => api.listSessionContexts(session!),
    enabled: !!session,
  });
  const treeQ = useQuery({
    queryKey: ["session-span-tree", session],
    queryFn: () => api.getSessionSpanTree(session!),
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
  }, [tick, qc, session]);

  // Pointer drag-resize from the top edge.
  const dragRef = useRef<{ startY: number; startVh: number } | null>(null);
  const onResizeDown = (e: React.PointerEvent) => {
    dragRef.current = { startY: e.clientY, startVh: heightVh };
    e.currentTarget.setPointerCapture(e.pointerId);
  };
  const onResizeMove = (e: React.PointerEvent) => {
    const d = dragRef.current;
    if (!d) return;
    const dy = e.clientY - d.startY;
    const next = d.startVh - (dy / window.innerHeight) * 100;
    setHeightVh(Math.min(80, Math.max(5, next)));
  };
  const onResizeUp = (e: React.PointerEvent) => {
    if (dragRef.current) {
      dragRef.current = null;
      e.currentTarget.releasePointerCapture(e.pointerId);
    }
  };

  const tree = treeQ.data?.tree ?? [];
  const chatByPk = useMemo(() => {
    const arr: ChatSpanInfo[] = [];
    collectChatSpans(tree, 0, arr);
    const m = new Map<number, ChatSpanInfo>();
    for (const c of arr) m.set(c.span_pk, c);
    return m;
  }, [tree]);

  const snapshots = contextsQ.data?.context_snapshots ?? [];
  const rows = useMemo(() => {
    // Merge snapshots per span_pk: a span typically gets one
    // `usage_info_event` snapshot (carrying token_limit + current_tokens)
    // and one `chat_span` snapshot (carrying input/output/reasoning
    // tokens). Combine them by taking the max non-null token_limit and
    // the latest non-null token-count fields per span.
    interface Merged extends MergedRow {}
    const byPk = new Map<number, Merged>();
    for (const s of snapshots) {
      if (s.span_pk == null) continue;
      if (!chatByPk.has(s.span_pk)) continue;
      const cur = byPk.get(s.span_pk) ?? {
        span_pk: s.span_pk,
        token_limit: null,
        input_tokens: null,
        output_tokens: null,
        reasoning_tokens: null,
        cache_read_tokens: null,
        latest_ns: -1,
      };
      if (s.token_limit != null && (cur.token_limit == null || s.token_limit > cur.token_limit)) {
        cur.token_limit = s.token_limit;
      }
      if (s.captured_ns > cur.latest_ns) {
        if (s.input_tokens != null) cur.input_tokens = s.input_tokens;
        if (s.output_tokens != null) cur.output_tokens = s.output_tokens;
        if (s.reasoning_tokens != null) cur.reasoning_tokens = s.reasoning_tokens;
        if (s.cache_read_tokens != null) cur.cache_read_tokens = s.cache_read_tokens;
        cur.latest_ns = s.captured_ns;
      } else {
        if (cur.input_tokens == null && s.input_tokens != null) cur.input_tokens = s.input_tokens;
        if (cur.output_tokens == null && s.output_tokens != null) cur.output_tokens = s.output_tokens;
        if (cur.reasoning_tokens == null && s.reasoning_tokens != null) cur.reasoning_tokens = s.reasoning_tokens;
        if (cur.cache_read_tokens == null && s.cache_read_tokens != null) cur.cache_read_tokens = s.cache_read_tokens;
      }
      byPk.set(s.span_pk, cur);
    }
    return [...byPk.values()]
      .map((m) => ({ m, info: chatByPk.get(m.span_pk)! }))
      .sort(
        (a, b) =>
          (a.info.start_unix_ns ?? 0) - (b.info.start_unix_ns ?? 0),
      );
  }, [snapshots, chatByPk]);

  const { maxLimit, yMax } = useMemo(() => {
    let ml = 0;
    let mc = 0;
    // maxCurrent is the max `current_tokens` reported by usage_info_event
    // snapshots (i.e., the largest observed context-window occupancy),
    // not the max bar height. Sub-agent chat spans don't emit
    // usage_info_event, so their per-call prompt sizes (which can be
    // millions of tokens because they replay history) don't dominate
    // the y-axis — they clip above the chart instead.
    for (const s of snapshots) {
      if (s.token_limit != null && s.token_limit > ml) ml = s.token_limit;
      if (s.current_tokens != null && s.current_tokens > mc) {
        mc = s.current_tokens;
      }
    }
    const yMax = ml > 0 ? Math.max(ml * 1.10, mc) : mc || 1;
    return { maxLimit: ml, yMax };
  }, [snapshots]);

  if (!visible) {
    return (
      <button
        className="ctx-collapsed"
        onClick={() => setVisible(true)}
        style={{
          position: "fixed",
          left: 6,
          bottom: 6,
          padding: "2px 8px",
          background: "var(--bg-1)",
          color: "var(--fg)",
          border: "1px solid var(--border)",
          borderRadius: 3,
          cursor: "pointer",
          zIndex: 50,
        }}
        title="show context growth"
      >
        ▾ context growth
      </button>
    );
  }

  return (
    <div
      className="ctx-widget"
      style={{
        position: "fixed",
        left: 0,
        right: 0,
        bottom: 0,
        height: `${heightVh}vh`,
        background: "var(--bg-1)",
        color: "var(--fg)",
        borderTop: "1px solid var(--border)",
        display: "flex",
        flexDirection: "column",
        zIndex: 40,
      }}
    >
      <div
        className="ctx-resizer"
        onPointerDown={onResizeDown}
        onPointerMove={onResizeMove}
        onPointerUp={onResizeUp}
        onPointerCancel={onResizeUp}
        style={{
          height: 6,
          cursor: "ns-resize",
          background: "var(--bg-2)",
          borderBottom: "1px solid var(--border)",
          flex: "0 0 auto",
        }}
      />
      <div
        className="ctx-header"
        style={{
          display: "flex",
          alignItems: "center",
          gap: 8,
          padding: "2px 6px",
          fontSize: 12,
          flex: "0 0 auto",
        }}
      >
        <span className="dim">context growth</span>
        {session && <span className="dim">— {session}</span>}
        <span style={{ flex: 1 }} />
        <Legend />
        <button
          onClick={() => setVisible(false)}
          title="hide"
          style={{
            background: "transparent",
            color: "var(--fg)",
            border: "1px solid var(--border)",
            cursor: "pointer",
            padding: "0 6px",
          }}
        >
          ×
        </button>
      </div>
      <div
        className="ctx-body"
        style={{ flex: 1, position: "relative", overflow: "hidden" }}
      >
        {!session ? (
          <div className="dim" style={{ padding: 8 }}>
            pick a session
          </div>
        ) : rows.length === 0 ? (
          <div className="dim" style={{ padding: 8 }}>
            no context snapshots yet
          </div>
        ) : (
          <Chart
            rows={rows}
            yMax={yMax}
            maxLimit={maxLimit}
            hoveredChatPk={hoveredChatPk}
          />
        )}
      </div>
    </div>
  );
}

interface MergedRow {
  span_pk: number;
  token_limit: number | null;
  input_tokens: number | null;
  output_tokens: number | null;
  reasoning_tokens: number | null;
  cache_read_tokens: number | null;
  latest_ns: number;
}

interface ChartProps {
  rows: { m: MergedRow; info: ChatSpanInfo }[];
  yMax: number;
  maxLimit: number;
  hoveredChatPk: number | null;
}

function Legend() {
  const Sw = ({ c }: { c: string }) => (
    <span
      style={{
        display: "inline-block",
        width: 10,
        height: 10,
        background: c,
        marginRight: 4,
        verticalAlign: "middle",
      }}
    />
  );
  return (
    <div
      className="ctx-legend"
      style={{ display: "flex", gap: 10, fontSize: 11, color: "var(--fg-dim)" }}
    >
      <span><Sw c="#60a5fa" />input</span>
      <span><Sw c="#fb923c" />output</span>
      <span><Sw c="#fde047" />reasoning</span>
      <span style={{ color: "#facc15" }}>┄ limit</span>
    </div>
  );
}

function formatTokens(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`;
  if (n >= 1_000) return `${(n / 1_000).toFixed(n >= 10_000 ? 0 : 1)}k`;
  return String(n);
}

function Chart({ rows, yMax, maxLimit, hoveredChatPk }: ChartProps) {
  const limitTopPct = (1 - maxLimit / yMax) * 100;
  const Y_AXIS_W = 44;
  const X_AXIS_H = 18;
  // 5 ticks: 0, 25%, 50%, 75%, 100%.
  const yTicks = [0, 0.25, 0.5, 0.75, 1].map((f) => ({
    pct: (1 - f) * 100,
    value: Math.round(yMax * f),
  }));
  return (
    <div
      style={{
        position: "absolute",
        inset: 0,
        display: "flex",
        flexDirection: "column",
      }}
    >
      <div
        style={{
          flex: 1,
          display: "flex",
          minHeight: 0,
        }}
      >
        {/* y-axis */}
        <div
          className="ctx-yaxis"
          style={{
            width: Y_AXIS_W,
            position: "relative",
            flex: "0 0 auto",
            fontSize: 10,
            color: "var(--fg-mute)",
          }}
        >
          {yTicks.map((t) => (
            <div
              key={t.value}
              style={{
                position: "absolute",
                right: 4,
                top: `${t.pct}%`,
                transform: "translateY(-50%)",
                whiteSpace: "nowrap",
              }}
            >
              {formatTokens(t.value)}
            </div>
          ))}
        </div>
        {/* plot area */}
        <div
          className="ctx-plot"
          style={{
            flex: 1,
            position: "relative",
            padding: "6px 8px 0 0",
            display: "flex",
            alignItems: "flex-end",
            justifyContent: "flex-start",
            gap: 2,
            minWidth: 0,
          }}
        >
          {maxLimit > 0 && (
            <div
              className="ctx-limit"
              style={{
                position: "absolute",
                left: 0,
                right: 0,
                top: `${limitTopPct}%`,
                height: 1,
                backgroundImage:
                  "linear-gradient(to right, #facc15 0, #facc15 10px, transparent 10px, transparent 16px)",
                backgroundSize: "16px 1px",
                backgroundRepeat: "repeat-x",
                pointerEvents: "none",
              }}
              title={`token_limit = ${maxLimit}`}
            />
          )}
          {rows.map(({ m, info }, i) => {
            const isSub = info.invokeAgentDepth > 1;
            const rawInp = m.input_tokens ?? 0;
            const cacheR = m.cache_read_tokens ?? 0;
            // For sub-agents, exclude cache_read_tokens (the replayed
            // cached prompt) so the bar reflects only fresh per-call
            // input. For the root agent, keep input_tokens as-is.
            const inp = isSub ? Math.max(0, rawInp - cacheR) : rawInp;
            const out = m.output_tokens ?? 0;
            const rea = m.reasoning_tokens ?? 0;
            const total = inp + out + rea;
            const prevIsSub =
              i > 0 ? rows[i - 1].info.invokeAgentDepth > 1 : isSub;
            const nextIsSub =
              i < rows.length - 1
                ? rows[i + 1].info.invokeAgentDepth > 1
                : isSub;
            const groupBreak = i > 0 && prevIsSub !== isSub;
            // Sub-agent tint extends 3px past the cell only at the run
            // boundaries; interior cells of a sub-agent run paint flush
            // so adjacent overlays don't double-brighten.
            const tintLeft = isSub && !prevIsSub ? -3 : 0;
            const tintRight = isSub && !nextIsSub ? -3 : 0;
            const isHovered = hoveredChatPk === info.span_pk;
            const colors = {
              input: "#60a5fa",
              output: "#fb923c",
              reasoning: "#fde047",
            };
            const cls =
              "ctx-bar-cell" +
              (isSub ? " subagent" : "") +
              (isHovered ? " hovered" : "");
            const heightPct = total > 0 ? (total / yMax) * 100 : 0;
            return (
              <div
                key={info.span_pk}
                className={cls}
                style={{
                  position: "relative",
                  flex: 1,
                  minWidth: 4,
                  maxWidth: 28,
                  height: "100%",
                  display: "flex",
                  flexDirection: "column-reverse",
                  marginLeft: groupBreak ? 6 : 0,
                  borderBottom: `4px solid ${
                    isHovered ? "#facc15" : "transparent"
                  }`,
                  boxSizing: "border-box",
                }}
                title={
                  `span_pk=${info.span_pk}\n` +
                  `input=${inp} (raw=${rawInp}, cache_read=${cacheR})\n` +
                  `output=${out}\n` +
                  `reasoning=${rea}\n` +
                  `total=${total}\n` +
                  `limit=${m.token_limit ?? "-"}` +
                  (isSub ? "\n(sub-agent)" : "")
                }
              >
                {isSub && (
                  <div
                    aria-hidden
                    style={{
                      position: "absolute",
                      top: 0,
                      bottom: 0,
                      left: tintLeft,
                      right: tintRight,
                      background: "rgba(255,255,255,0.08)",
                      pointerEvents: "none",
                      zIndex: 0,
                    }}
                  />
                )}
                <div
                  style={{
                    position: "relative",
                    zIndex: 1,
                    height: `${heightPct}%`,
                    display: "flex",
                    flexDirection: "column-reverse",
                  }}
                >
                  {total > 0 && (
                    <>
                      <div
                        style={{
                          height: `${(inp / total) * 100}%`,
                          background: colors.input,
                        }}
                      />
                      <div
                        style={{
                          height: `${(out / total) * 100}%`,
                          background: colors.output,
                        }}
                      />
                      <div
                        style={{
                          height: `${(rea / total) * 100}%`,
                          background: colors.reasoning,
                        }}
                      />
                    </>
                  )}
                </div>
              </div>
            );
          })}
        </div>
      </div>
      {/* x-axis scale */}
      <div
        className="ctx-xaxis"
        style={{
          height: X_AXIS_H,
          display: "flex",
          alignItems: "center",
          fontSize: 10,
          color: "var(--fg-mute)",
          borderTop: "1px solid var(--border)",
          flex: "0 0 auto",
        }}
      >
        <div style={{ width: Y_AXIS_W, textAlign: "right", paddingRight: 4 }}>
          turn
        </div>
        <div
          style={{
            flex: 1,
            display: "flex",
            justifyContent: "space-between",
            padding: "0 8px",
          }}
        >
          <span>1</span>
          {rows.length > 2 && <span>{Math.ceil(rows.length / 2)}</span>}
          <span>{rows.length}</span>
        </div>
      </div>
    </div>
  );
}
