import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useCallback, useEffect, useMemo, useRef, useState, type KeyboardEvent } from "react";
import { api } from "../api/client";
import type { Column } from "../state/workspace";
import { useWorkspace } from "../state/workspace";
import { ColumnHeader } from "../components/ColumnHeader";
import { useLiveFeed } from "../state/live";
import { useHoverState } from "../state/hover";
import { fmtNs, fmtClock, parseToolCallArguments } from "../components/content";
import { kindLabel, kindClass as kindCls, HashTag, RollingDots } from "../components/KindBadge";
import type {
  KindClass,
  SpanNode,
  SpanProjection,
  TraceSummary,
} from "../api/types";

const KINDS: Array<KindClass | ""> = [
  "",
  "invoke_agent",
  "chat",
  "execute_tool",
  "external_tool",
  "other",
];

// Sort key for sibling ordering: completion-time ascending, with
// start_unix_ns and span_pk as tie-breakers. Mirrors the convention
// used by ChatDetail's flatChatSpansSorted so cross-column
// selection feels consistent.
function sortKey(n: SpanNode): number {
  return n.end_unix_ns ?? n.start_unix_ns ?? n.span_pk ?? 0;
}

// Locate the picked span in the loaded session tree and return its
// sibling array (children of its parent, or the top-level array when
// the picked span is a root). Returns null when the span isn't found.
function findSiblings(tree: SpanNode[], span_id: string): { picked: SpanNode; siblings: SpanNode[] } | null {
  const find = (nodes: SpanNode[], parentSiblings: SpanNode[]): { picked: SpanNode; siblings: SpanNode[] } | null => {
    for (const n of nodes) {
      if (n.span_id === span_id) return { picked: n, siblings: parentSiblings };
      const hit = find(n.children ?? [], n.children ?? []);
      if (hit) return hit;
    }
    return null;
  };
  return find(tree, tree);
}

// Among the siblings of the picked tool span, find the first chat
// span whose completion-time sort key is strictly greater than the
// picked span's. Returns its span_id, or undefined if no such chat
// sibling exists in the loaded tree.
function findNextChatSiblingId(tree: SpanNode[], span_id: string): string | undefined {
  const hit = findSiblings(tree, span_id);
  if (!hit) return undefined;
  const pickedKey = sortKey(hit.picked);
  const ordered = [...hit.siblings].sort((a, b) => {
    const ka = sortKey(a);
    const kb = sortKey(b);
    if (ka !== kb) return ka - kb;
    return (a.span_pk ?? 0) - (b.span_pk ?? 0);
  });
  for (const sib of ordered) {
    if (sib.kind_class !== "chat") continue;
    const sk = sortKey(sib);
    if (sk > pickedKey || (sk === pickedKey && (sib.span_pk ?? 0) > (hit.picked.span_pk ?? 0))) {
      return sib.span_id;
    }
  }
  return undefined;
}

function flattenSpanTree(tree: SpanNode[], collapsed?: Set<string>): SpanNode[] {
  const rows: SpanNode[] = [];
  const walk = (nodes: SpanNode[]) => {
    for (const n of nodes) {
      rows.push(n);
      if (collapsed?.has(n.span_id)) continue;
      walk(n.children ?? []);
    }
  };
  walk(tree);
  return rows;
}

// Build a map from each span_id to its parent span_id in the tree.
function buildParentMap(tree: SpanNode[]): Map<string, string> {
  const m = new Map<string, string>();
  const walk = (nodes: SpanNode[], parentId: string | null) => {
    for (const n of nodes) {
      if (parentId) m.set(n.span_id, parentId);
      walk(n.children ?? [], n.span_id);
    }
  };
  walk(tree, null);
  return m;
}

// Collect all ancestor span_ids of a set of target span_ids.
function collectAncestors(targets: Iterable<string>, parentMap: Map<string, string>): Set<string> {
  const ancestors = new Set<string>();
  for (const id of targets) {
    let cur = parentMap.get(id);
    while (cur) {
      if (ancestors.has(cur)) break;
      ancestors.add(cur);
      cur = parentMap.get(cur);
    }
  }
  return ancestors;
}

// Check if targetId is a descendant of ancestorId in the tree.
function isDescendant(targetId: string, ancestorId: string, parentMap: Map<string, string>): boolean {
  let cur = parentMap.get(targetId);
  while (cur) {
    if (cur === ancestorId) return true;
    cur = parentMap.get(cur);
  }
  return false;
}

// Trace-centric scenario.
//
// Two modes, gated by whether a session (conversation_id) is selected:
//
//   - No session:  live list of traces (/api/traces). Useful when the
//     conversation_id is not yet known (no chat span has landed).
//   - Session set: full session span tree (/api/sessions/:cid/span-tree)
//     rendered as one expandable tree. All spans associated with that
//     conversation are shown together; the tree updates dynamically as
//     new spans arrive over the WS feed.
//
// Selecting a span (or a trace row when no session is set) propagates
// (selected_trace_id, selected_span_id) to all linked columns so the
// inspector / tool detail / shell I/O views render in lock-step.
export function SpansScenario({ column }: { column: Column }) {
  const qc = useQueryClient();
  const updateColumn = useWorkspace((s) => s.updateColumn);
  const columns = useWorkspace((s) => s.columns);
  const { session, selected_span_id, kind_filter } = column.config;

  const sessionsQ = useQuery({
    queryKey: ["sessions"],
    queryFn: () => api.listSessions({ limit: 100 }),
  });

  const tracesQ = useQuery({
    queryKey: ["traces"],
    queryFn: () => api.listTraces({ limit: 100 }),
    enabled: !session,
  });

  const sessionTreeQ = useQuery({
    queryKey: ["session-span-tree", session],
    queryFn: () => api.getSessionSpanTree(session!),
    enabled: !!session,
  });

  // Live invalidation. The backend emits kind:"trace" on every span
  // insert/upgrade and on placeholder creation, plus derived envelopes
  // when projections (chat_turn, tool_call, agent_run) land. Any of
  // these can change which spans belong to a session, so invalidate the
  // session tree on every ingest event while a session is selected.
  const { tick } = useLiveFeed([
    { kind: "trace", entity: "trace" },
    { kind: "span", entity: "span" },
    { kind: "span", entity: "placeholder" },
    { kind: "derived", entity: "tool_call" },
    { kind: "derived", entity: "chat_turn" },
    { kind: "derived", entity: "agent_run" },
    { kind: "derived", entity: "session" },
  ]);
  useEffect(() => {
    qc.invalidateQueries({ queryKey: ["sessions"] });
    if (session) {
      qc.invalidateQueries({ queryKey: ["session-span-tree", session] });
    } else {
      qc.invalidateQueries({ queryKey: ["traces"] });
    }
  }, [tick, qc, session]);

  const traces = tracesQ.data?.traces ?? [];
  const tree = sessionTreeQ.data?.tree ?? [];

  // --- search state ---
  const [searchText, setSearchText] = useState("");
  const [debouncedSearch, setDebouncedSearch] = useState("");
  const debounceRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  const onSearchChange = useCallback((text: string) => {
    setSearchText(text);
    if (debounceRef.current) clearTimeout(debounceRef.current);
    debounceRef.current = setTimeout(() => setDebouncedSearch(text), 300);
  }, []);

  // Clear search when session is deselected.
  useEffect(() => {
    if (!session) {
      setSearchText("");
      setDebouncedSearch("");
    }
  }, [session]);

  const searchQ = useQuery({
    queryKey: ["search-spans", session, debouncedSearch],
    queryFn: () => api.searchSpans({ q: debouncedSearch, session: session! }),
    enabled: !!session && debouncedSearch.length > 0,
  });

  const searchHitSpanIds: Set<string> | null = useMemo(() => {
    if (!session || !debouncedSearch || !searchQ.data) return null;
    return new Set(searchQ.data.results.map((r) => r.span_id));
  }, [session, debouncedSearch, searchQ.data]);

  // Propagate the debounced search query to sibling detail columns so
  // they can drive their own highlighting / auto-expand behavior.
  useEffect(() => {
    columns.forEach((c) => {
      if (c.scenarioType !== "chat_detail" && c.scenarioType !== "tool_detail") return;
      const prev = (c.config.search_query as string | undefined) ?? "";
      const next = debouncedSearch;
      if (prev === next) return;
      updateColumn(c.id, { config: { ...c.config, search_query: next } });
    });
  }, [debouncedSearch, columns, updateColumn]);

  // Applicability map: which scenario types accept selections from
  // which span kinds. Selecting a chat span only updates input-breakdown
  // (and the spans column itself); selecting a tool span only updates
  // tool-detail. Non-matching columns keep their last applicable
  // selection.
  const SCENARIO_KINDS: Record<string, KindClass[] | "*"> = {
    spans: "*",
    tool_detail: ["execute_tool", "external_tool"],
    chat_detail: ["chat"],
  };

  const onPickSpan = (trace_id: string, span_id: string, kind_class: KindClass) => {
    // For execute_tool selections, also auto-advance chat_detail
    // columns to the chat span that immediately follows the picked
    // tool span among its siblings (same parent_span_id) when one
    // exists in the loaded session tree. Tool-kind selections would
    // otherwise leave chat_detail stuck on a stale chat span.
    let nextChatSpanId: string | undefined;
    let toolCallId: string | undefined;
    if (kind_class === "execute_tool" && tree.length > 0) {
      const hit = findSiblings(tree, span_id);
      toolCallId = hit?.picked.projection.tool_call?.call_id ?? undefined;
      nextChatSpanId = findNextChatSiblingId(tree, span_id);
    }

    columns.forEach((c) => {
      const allowed = SCENARIO_KINDS[c.scenarioType];
      if (!allowed) return;
      const accepts = allowed === "*" || allowed.includes(kind_class);
      if (accepts) {
        const patch: Record<string, unknown> = {
          ...c.config,
          selected_trace_id: trace_id,
          selected_span_id: span_id,
        };
        // Direct chat / non-tool selections clear any prior
        // tool-driven hint so the arrow doesn't linger.
        if (c.scenarioType === "chat_detail") {
          patch.selected_tool_call_id = undefined;
        }
        updateColumn(c.id, { config: patch });
        return;
      }
      if (c.scenarioType === "chat_detail" && nextChatSpanId) {
        updateColumn(c.id, {
          config: {
            ...c.config,
            selected_trace_id: trace_id,
            selected_span_id: nextChatSpanId,
            selected_tool_call_id: toolCallId,
          },
        });
      }
    });
  };

  const onPickTrace = (trace_id: string, span_id?: string, kind_class?: KindClass) => {
    if (!span_id) return;
    onPickSpan(trace_id, span_id, kind_class ?? "other");
  };

  // "Follow latest tool call" convenience: if the user is currently
  // sitting on what was the most-recent tool span, auto-advance the
  // selection forward as new tool spans arrive. Once the user picks
  // anything other than the latest, this disengages until they
  // re-select the latest manually.
  const TOOL_KINDS: KindClass[] = ["execute_tool", "external_tool"];
  const latestToolSpan = useMemo(() => {
    let best: SpanNode | null = null;
    const walk = (nodes: SpanNode[]) => {
      for (const n of nodes) {
        if (TOOL_KINDS.includes(n.kind_class)) {
          const bk = best ? (best.start_unix_ns ?? best.span_pk ?? 0) : -1;
          const nk = n.start_unix_ns ?? n.span_pk ?? 0;
          if (!best || nk > bk) best = n;
        }
        walk(n.children ?? []);
      }
    };
    walk(tree);
    return best as SpanNode | null;
  }, [tree]);

  const prevLatestToolIdRef = useRef<string | undefined>(undefined);
  useEffect(() => {
    const latestId = latestToolSpan?.span_id;
    const prev = prevLatestToolIdRef.current;
    if (
      latestToolSpan &&
      latestId &&
      prev &&
      latestId !== prev &&
      selected_span_id === prev
    ) {
      onPickSpan(latestToolSpan.trace_id, latestId, latestToolSpan.kind_class);
    }
    prevLatestToolIdRef.current = latestId;
    // onPickSpan is intentionally omitted — it closes over `columns`
    // and recreates each render; we only care about advances driven by
    // tree updates and selection changes.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [latestToolSpan, selected_span_id]);

  // Consume click-from-widget signal: when the context growth chart bar
  // is clicked, select the corresponding chat span in the tree.
  const clickedChat = useHoverState((s) => s.clickedChat);
  const setClickedChat = useHoverState((s) => s.setClickedChat);
  useEffect(() => {
    if (!clickedChat) return;
    onPickSpan(clickedChat.traceId, clickedChat.spanId, "chat");
    setClickedChat(null);
    // onPickSpan omitted — same rationale as above.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [clickedChat, setClickedChat]);

  return (
    <>
      <ColumnHeader column={column}>
        <span className="dim">session</span>
        <select
          value={session ?? ""}
          onChange={(e) => {
            const next = e.target.value || undefined;
            updateColumn(column.id, {
              config: {
                ...column.config,
                session: next,
                selected_trace_id: undefined,
                selected_span_id: undefined,
              },
            });
            // Mirror LiveSessions.onSelect: propagate the session change
            // to sibling columns whose scenarios are session-scoped, so
            // FileTouches / ChatDetail / other Spans columns stay in sync.
            columns.forEach((c) => {
              if (c.id === column.id) return;
              if (
                ["spans", "chat_detail", "file_touches"].includes(c.scenarioType)
              ) {
                updateColumn(c.id, { config: { ...c.config, session: next } });
              }
            });
          }}
        >
          <option value="">all</option>
          {sessionsQ.data?.sessions.map((s) => {
            const shortId = s.conversation_id.slice(0, 8);
            const name =
              s.local_name && s.local_name.trim().length > 0 ? s.local_name : null;
            return (
              <option key={s.conversation_id} value={s.conversation_id}>
                {name ? `${name} · ${shortId}` : shortId}
              </option>
            );
          })}
        </select>
        <span className="dim">kind</span>
        <select
          value={kind_filter ?? ""}
          onChange={(e) =>
            updateColumn(column.id, {
              config: {
                ...column.config,
                kind_filter: (e.target.value || undefined) as KindClass | undefined,
              },
            })
          }
        >
          {KINDS.map((k) => (
            <option key={k} value={k}>
              {k ? kindLabel(k as KindClass) : "any"}
            </option>
          ))}
        </select>
        {session && (
          <input
            type="text"
            placeholder="search…"
            value={searchText}
            onChange={(e) => onSearchChange(e.target.value)}
            style={{ marginLeft: 6, minWidth: 80, flex: "1 1 auto" }}
          />
        )}
      </ColumnHeader>
      <div className="col-body list" style={{ overflow: "auto" }}>
        {session ? (
          <SpanTreeView
            key={session}
            tree={tree}
            loading={sessionTreeQ.isLoading}
            kindFilter={kind_filter}
            selectedSpanId={selected_span_id}
            onSelect={onPickSpan}
            searchHitSpanIds={searchHitSpanIds}
          />
        ) : (
          <TracesList
            rows={traces}
            loading={tracesQ.isLoading}
            kindFilter={kind_filter}
            selectedSpanId={selected_span_id}
            onSelect={onPickTrace}
          />
        )}
      </div>
    </>
  );
}

// --- traces list ------------------------------------------------------

function TracesList({
  rows,
  loading,
  kindFilter,
  selectedSpanId,
  onSelect,
}: {
  rows: TraceSummary[];
  loading: boolean;
  kindFilter: KindClass | undefined;
  selectedSpanId: string | undefined;
  onSelect: (trace_id: string, span_id?: string, kind_class?: KindClass) => void;
}) {
  // The kind filter dims traces that have zero spans of that kind so the
  // user keeps situational awareness. It does not hide them — partial
  // ingest states would be impossible to reason about otherwise.
  const decorated = useMemo(
    () =>
      rows.map((r) => ({
        r,
        dim: !!kindFilter && (r.kind_counts[kindFilter] ?? 0) === 0,
      })),
    [rows, kindFilter]
  );

  const selectableRows = useMemo(
    () => decorated.filter(({ r }) => r.root?.span_id),
    [decorated]
  );

  const onKeyDown = (e: KeyboardEvent<HTMLDivElement>) => {
    if (e.key !== "ArrowDown" && e.key !== "ArrowUp") return;
    e.preventDefault();
    if (selectableRows.length === 0) return;

    const current = selectableRows.findIndex(
      ({ r }) => r.root?.span_id === selectedSpanId
    );
    const nextIndex =
      e.key === "ArrowDown"
        ? current < 0
          ? 0
          : Math.min(current + 1, selectableRows.length - 1)
        : current < 0
          ? selectableRows.length - 1
          : Math.max(current - 1, 0);
    const next = selectableRows[nextIndex].r.root;
    if (!next || next.span_id === selectedSpanId) return;
    onSelect(selectableRows[nextIndex].r.trace_id, next.span_id, next.kind_class);
  };

  if (loading) return <div className="empty-state">loading…</div>;
  if (rows.length === 0)
    return (
      <div className="empty-state">
        no traces yet — start a Copilot CLI session that exports OTLP to
        this collector
      </div>
    );

  return (
    <div
      tabIndex={0}
      onMouseDown={(e) => e.currentTarget.focus({ preventScroll: true })}
      onKeyDown={onKeyDown}
    >
      {decorated.map(({ r, dim }) => {
        const dur =
          r.first_seen_ns != null && r.last_seen_ns != null
            ? r.last_seen_ns - r.first_seen_ns
            : null;
        const rootName = r.root?.name ?? "(unknown root)";
        const live = r.placeholder_count > 0;
        return (
          <div
            key={r.trace_id}
            className={`row${r.root?.span_id === selectedSpanId ? " sel" : ""}${dim ? " dim" : ""}`}
            onClick={() => onSelect(r.trace_id, r.root?.span_id, r.root?.kind_class)}
          >
            <span className="pri mono">{rootName}</span>
            <span className="sec mono">{r.trace_id.slice(0, 8)}</span>
            <KindCountChips counts={r.kind_counts} />
            {live && <span className="tag warn">live · {r.placeholder_count} ph</span>}
            {r.conversation_id && (
              <span className="tag">conv {r.conversation_id.slice(0, 6)}</span>
            )}
            <span className="sec">{r.span_count} spans</span>
            <span className="sec">{fmtNs(dur)}</span>
            <span className="right dim">{fmtClock(r.last_seen_ns)}</span>
          </div>
        );
      })}
    </div>
  );
}

function KindCountChips({ counts }: { counts: TraceSummary["kind_counts"] }) {
  const order: Array<keyof TraceSummary["kind_counts"]> = [
    "chat",
    "execute_tool",
    "external_tool",
    "invoke_agent",
    "other",
  ];
  return (
    <span style={{ marginLeft: 4 }}>
      {order
        .filter((k) => counts[k] > 0)
        .map((k) => (
          <span key={k} className={kindCls(k as KindClass)} style={{ marginRight: 4 }}>
            {kindLabel(k as KindClass)} {counts[k]}
          </span>
        ))}
    </span>
  );
}

// --- per-trace tree ---------------------------------------------------

function SpanTreeView({
  tree,
  loading,
  kindFilter,
  selectedSpanId,
  onSelect,
  searchHitSpanIds,
}: {
  tree: SpanNode[];
  loading: boolean;
  kindFilter: KindClass | undefined;
  selectedSpanId: string | undefined;
  onSelect: (t: string, s: string, k: KindClass) => void;
  searchHitSpanIds: Set<string> | null;
}) {
  const [userCollapsed, setUserCollapsed] = useState<Set<string>>(new Set());

  const parentMap = useMemo(() => buildParentMap(tree), [tree]);

  // Ancestors that must be expanded for search hits + selection to be visible.
  const forcedExpandAncestors = useMemo(() => {
    const targets: string[] = [];
    if (searchHitSpanIds) {
      for (const id of searchHitSpanIds) targets.push(id);
    }
    if (selectedSpanId) targets.push(selectedSpanId);
    if (targets.length === 0) return new Set<string>();
    return collectAncestors(targets, parentMap);
  }, [searchHitSpanIds, selectedSpanId, parentMap]);

  const effectiveCollapsed = useMemo(() => {
    if (forcedExpandAncestors.size === 0) return userCollapsed;
    const out = new Set<string>();
    for (const id of userCollapsed) {
      if (!forcedExpandAncestors.has(id)) out.add(id);
    }
    return out;
  }, [userCollapsed, forcedExpandAncestors]);

  const toggleCollapse = useCallback(
    (span_id: string) => {
      setUserCollapsed((prev) => {
        const next = new Set(prev);
        if (next.has(span_id)) {
          next.delete(span_id);
        } else {
          next.add(span_id);
          // If the selected span is inside the subtree being collapsed,
          // move selection to the collapsing node.
          if (selectedSpanId && isDescendant(selectedSpanId, span_id, parentMap)) {
            // Find the collapsing node to get its trace_id and kind_class.
            const flat = flattenSpanTree(tree);
            const node = flat.find((n) => n.span_id === span_id);
            if (node) onSelect(node.trace_id, node.span_id, node.kind_class);
          }
        }
        return next;
      });
    },
    // onSelect/parentMap/tree recreate each render; only selection identity matters.
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [selectedSpanId, parentMap, tree],
  );

  const rows = useMemo(
    () => flattenSpanTree(tree, effectiveCollapsed),
    [tree, effectiveCollapsed],
  );

  const onKeyDown = (e: KeyboardEvent<HTMLDivElement>) => {
    if (e.key !== "ArrowDown" && e.key !== "ArrowUp") return;
    e.preventDefault();
    if (rows.length === 0) return;

    const current = rows.findIndex((n) => n.span_id === selectedSpanId);
    const nextIndex =
      e.key === "ArrowDown"
        ? current < 0
          ? 0
          : Math.min(current + 1, rows.length - 1)
        : current < 0
          ? rows.length - 1
          : Math.max(current - 1, 0);
    const next = rows[nextIndex];
    if (!next || next.span_id === selectedSpanId) return;
    onSelect(next.trace_id, next.span_id, next.kind_class);
  };

  if (loading) return <div className="empty-state">loading…</div>;
  if (tree.length === 0) return <div className="empty-state">no spans in trace</div>;
  return (
    <div
      tabIndex={0}
      onMouseDown={(e) => e.currentTarget.focus({ preventScroll: true })}
      onKeyDown={onKeyDown}
    >
      {tree.map((n) => (
        <SpanTreeNode
          key={n.span_pk}
          node={n}
          depth={0}
          kindFilter={kindFilter}
          selectedSpanId={selectedSpanId}
          onSelect={onSelect}
          chatAncestorPk={null}
          searchHitSpanIds={searchHitSpanIds}
          effectiveCollapsed={effectiveCollapsed}
          toggleCollapse={toggleCollapse}
        />
      ))}
    </div>
  );
}

function SpanTreeNode({
  node,
  depth,
  kindFilter,
  selectedSpanId,
  onSelect,
  chatAncestorPk,
  searchHitSpanIds,
  effectiveCollapsed,
  toggleCollapse,
}: {
  node: SpanNode;
  depth: number;
  kindFilter: KindClass | undefined;
  selectedSpanId: string | undefined;
  onSelect: (t: string, s: string, k: KindClass) => void;
  chatAncestorPk: number | null;
  searchHitSpanIds: Set<string> | null;
  effectiveCollapsed: Set<string>;
  toggleCollapse: (span_id: string) => void;
}) {
  const isSearchActive = searchHitSpanIds !== null;
  const isSearchHit = isSearchActive && searchHitSpanIds.has(node.span_id);
  const searchMiss = isSearchActive && !isSearchHit;
  const dim = !isSearchActive && !!kindFilter && node.kind_class !== kindFilter;
  const sel = selectedSpanId === node.span_id;
  const dur =
    node.start_unix_ns != null && node.end_unix_ns != null
      ? node.end_unix_ns - node.start_unix_ns
      : null;
  const setHoveredChatPk = useHoverState((s) => s.setHoveredChatPk);
  const hoverChatPk =
    node.kind_class === "chat" ? node.span_pk : chatAncestorPk;
  const childChatAncestorPk =
    node.kind_class === "chat" ? node.span_pk : chatAncestorPk;
  const rowRef = useRef<HTMLDivElement | null>(null);
  useEffect(() => {
    if (sel) rowRef.current?.scrollIntoView({ block: "nearest" });
  }, [sel]);
  const hasChildren = node.children.length > 0;
  const collapsed = effectiveCollapsed.has(node.span_id);
  return (
    <div>
      <div
        ref={rowRef}
        className={`row${sel ? " sel" : ""}${isSearchHit ? " search-hit" : ""}${searchMiss ? " search-miss" : ""}${dim ? " dim" : ""}`}
        style={{ paddingLeft: depth * 12 + 6 }}
        onClick={() => onSelect(node.trace_id, node.span_id, node.kind_class)}
        onMouseEnter={() => setHoveredChatPk(hoverChatPk ?? null)}
        onMouseLeave={() => setHoveredChatPk(null)}
      >
        <span className={kindCls(node.kind_class)}>{kindLabel(node.kind_class)}</span>
        {node.ingestion_state === "placeholder" && (
          <span className="tag warn"><RollingDots /></span>
        )}
        <ProjectionChips projection={node.projection} />
        {node.projection?.tool_call?.tool_name === "bash" && (
          <BashCommandChip trace_id={node.trace_id} span_id={node.span_id} />
        )}
        {node.projection?.tool_call?.tool_name === "skill" && (
          <SkillNameChip trace_id={node.trace_id} span_id={node.span_id} />
        )}
        <TargetBadge trace_id={node.trace_id} span_id={node.span_id} />
        <ReportIntentTitle nodes={node.children} />
        <span className="sec">{fmtNs(dur)}</span>
        <span className="right dim">{fmtClock(node.start_unix_ns)}</span>
        {hasChildren ? (
          <button
            className="row-action"
            onClick={(e) => {
              e.stopPropagation();
              toggleCollapse(node.span_id);
            }}
            aria-label={collapsed ? "expand" : "collapse"}
            style={collapsed ? { display: "inline-block", transform: "rotate(-90deg)" } : undefined}
          >
            ▾
          </button>
        ) : (
          <span className="row-action" style={{ visibility: "hidden" }}>▾</span>
        )}
      </div>
      {!collapsed &&
        node.children.map((c) => (
          <SpanTreeNode
            key={c.span_pk}
            node={c}
            depth={depth + 1}
            kindFilter={kindFilter}
            selectedSpanId={selectedSpanId}
            onSelect={onSelect}
            chatAncestorPk={childChatAncestorPk}
            searchHitSpanIds={searchHitSpanIds}
            effectiveCollapsed={effectiveCollapsed}
            toggleCollapse={toggleCollapse}
          />
        ))}
    </div>
  );
}

export function ProjectionChips({ projection }: { projection: SpanProjection | null | undefined }) {
  if (!projection) return null;
  const plain: { label: string; cls?: string }[] = [];
  const hashed: string[] = [];
  if (projection.chat_turn) {
    const ct = projection.chat_turn;
    const tok = `${ct.input_tokens ?? "?"}/${ct.output_tokens ?? "?"}`;
    plain.push({ label: `tokens ${tok}` });
    if (ct.model) plain.push({ label: ct.model });
  }
  if (projection.tool_call) {
    const tc = projection.tool_call;
    if (tc.tool_name) hashed.push(tc.tool_name);
    if (tc.status_code != null && tc.status_code !== 0)
      plain.push({ label: `err ${tc.status_code}`, cls: "err" });
  }
  if (projection.agent_run) {
    const ar = projection.agent_run;
    if (ar.agent_name) plain.push({ label: `agent ${ar.agent_name}` });
  }
  if (projection.external_tool_call) {
    const ext = projection.external_tool_call;
    if (ext.tool_name) hashed.push(ext.tool_name);
  }
  if (plain.length === 0 && hashed.length === 0) return null;
  return (
    <span style={{ marginLeft: 4 }}>
      {hashed.map((c) => (
        <HashTag key={`h-${c}`} label={c} />
      ))}
      {plain.length > 0 && (
        <span className="dim">
          {plain.map((c, i) => (
            <span
              key={`p-${i}`}
              className={c.cls ? `tag ${c.cls}` : "tag"}
              style={{ marginRight: 4 }}
            >
              {c.label}
            </span>
          ))}
        </span>
      )}
    </span>
  );
}

// Extract the "primary command" words from a shell command string.
// Splits the command on `&&`, `||`, and `|` — but only when surrounded
// by whitespace, so we don't false-match characters inside quoted
// arguments, regexes, etc. Then runs the single-segment extraction on
// each part: skip leading `VAR=value` env assignments, take the first
// token, basename it. Empty/unparseable segments are dropped.
//
// Examples:
//   "ls -la"                       -> ["ls"]
//   "/usr/bin/python3 x.py"        -> ["python3"]
//   "FOO=bar jq ."                 -> ["jq"]
//   "cd /tmp && ls -la && jq ."    -> ["cd", "ls", "jq"]
//   "cat f.json | jq . | head"     -> ["cat", "jq", "head"]
//   "echo a||b"                    -> ["echo"]   (no spaces, ignored)
function shellCommandWords(s: string): string[] {
  const out: string[] = [];
  // Lookbehind/lookahead require whitespace on both sides of the
  // separator. `\|\|?` is greedy so `||` is consumed as one separator
  // (not two `|`s).
  for (const seg of s.split(/(?<=\s)(?:&&|\|\|?)(?=\s)/)) {
    const w = firstWordOfSegment(seg);
    if (w) out.push(w);
  }
  return out;
}
function firstWordOfSegment(seg: string): string | null {
  const trimmed = seg.trim();
  if (!trimmed) return null;
  const tokens = trimmed.split(/\s+/);
  let i = 0;
  while (i < tokens.length && /^[A-Za-z_][A-Za-z0-9_]*=/.test(tokens[i])) i++;
  const tok = tokens[i];
  if (!tok) return null;
  const base = tok.split("/").pop() ?? tok;
  if (!base) return null;
  return base.length > 24 ? base.slice(0, 24) + "…" : base;
}

// When a span has a `report_intent` tool_call child, pull its `intent`
// argument and render it as title text on the parent row. If multiple
// report_intent children exist (rare), pick the latest by start time
// — that's the current intent at this level.
function ReportIntentTitle({ nodes }: { nodes: SpanNode[] }) {
  const intentNode = useMemo(() => {
    let best: SpanNode | null = null;
    for (const n of nodes) {
      if (n.projection?.tool_call?.tool_name !== "report_intent") continue;
      const bk = best ? (best.start_unix_ns ?? best.span_pk ?? 0) : -1;
      const nk = n.start_unix_ns ?? n.span_pk ?? 0;
      if (!best || nk > bk) best = n;
    }
    return best;
  }, [nodes]);
  if (!intentNode) return null;
  return (
    <ReportIntentText
      trace_id={intentNode.trace_id}
      span_id={intentNode.span_id}
    />
  );
}

function ReportIntentText({
  trace_id,
  span_id,
}: {
  trace_id: string;
  span_id: string;
}) {
  const q = useQuery({
    queryKey: ["span", trace_id, span_id],
    queryFn: () => api.getSpan(trace_id, span_id),
    enabled: !!trace_id && !!span_id,
    staleTime: 30_000,
  });
  if (!q.data) return null;
  const args = parseToolCallArguments(q.data.span.attributes ?? {});
  if (!args || typeof args !== "object" || Array.isArray(args)) return null;
  const intent = (args as Record<string, unknown>).intent;
  if (typeof intent !== "string" || !intent) return null;
  return (
    <span style={{ marginLeft: 6, color: "#fff" }}>{intent}</span>
  );
}

// Renders one hash-colored chicklet per primary command word in a bash
// tool call's arguments (split on `&&`). Fetches the span detail
// (cached and shared with FileTouches/ToolDetail/ChatDetail via the
// ["span", trace_id, span_id] query key) and parses
// gen_ai.tool.call.arguments.command.
function BashCommandChip({ trace_id, span_id }: { trace_id: string; span_id: string }) {
  const q = useQuery({
    queryKey: ["span", trace_id, span_id],
    queryFn: () => api.getSpan(trace_id, span_id),
    enabled: !!trace_id && !!span_id,
    staleTime: 30_000,
  });
  if (!q.data) return null;
  const args = parseToolCallArguments(q.data.span.attributes ?? {});
  if (!args || typeof args !== "object" || Array.isArray(args)) return null;
  const cmd = (args as Record<string, unknown>).command;
  if (typeof cmd !== "string") return null;
  const words = shellCommandWords(cmd);
  if (words.length === 0) return null;
  const MAX = 6;
  const shown = words.slice(0, MAX);
  const overflow = words.length > MAX;
  return (
    <>
      {shown.map((w, i) => (
        <HashTag key={`${i}-${w}`} label={w} />
      ))}
      {overflow && <span className="tag" style={{ marginRight: 4 }}>…</span>}
    </>
  );
}

// Renders a green "skill" badge with the invoked skill's name. Pulls the
// span's full attributes (cached under the same ["span", trace_id, span_id]
// query key as BashCommandChip) and parses gen_ai.tool.call.arguments.skill.
function SkillNameChip({ trace_id, span_id }: { trace_id: string; span_id: string }) {
  const q = useQuery({
    queryKey: ["span", trace_id, span_id],
    queryFn: () => api.getSpan(trace_id, span_id),
    enabled: !!trace_id && !!span_id,
    staleTime: 30_000,
  });
  if (!q.data) return null;
  const args = parseToolCallArguments(q.data.span.attributes ?? {});
  if (!args || typeof args !== "object" || Array.isArray(args)) return null;
  const skill = (args as Record<string, unknown>).skill;
  if (typeof skill !== "string" || skill.length === 0) return null;
  return <span className="tag skill" style={{ marginRight: 4 }}>{skill}</span>;
}

// Renders a white-outlined badge showing the file name (basename) when the
// tool call's arguments contain a "path" property, or the domain when they
// contain a "url" property.
function TargetBadge({ trace_id, span_id }: { trace_id: string; span_id: string }) {
  const q = useQuery({
    queryKey: ["span", trace_id, span_id],
    queryFn: () => api.getSpan(trace_id, span_id),
    enabled: !!trace_id && !!span_id,
    staleTime: 30_000,
  });
  if (!q.data) return null;
  const args = parseToolCallArguments(q.data.span.attributes ?? {});
  if (!args || typeof args !== "object" || Array.isArray(args)) return null;
  const rec = args as Record<string, unknown>;

  // Try "path" → show basename
  const pathVal = rec.path;
  if (typeof pathVal === "string" && pathVal.length > 0) {
    const fileName = pathVal.split("/").pop() ?? pathVal;
    if (fileName) {
      return (
        <span
          className="tag"
          style={{ color: "#fff", borderColor: "#fff", marginRight: 4 }}
        >
          {fileName}
        </span>
      );
    }
  }

  // Try "url" → show domain only (no scheme, no path)
  const urlVal = rec.url;
  if (typeof urlVal === "string" && urlVal.length > 0) {
    try {
      const domain = new URL(urlVal).hostname;
      if (domain) {
        return (
          <span
            className="tag"
            style={{ color: "#fff", borderColor: "#fff", marginRight: 4 }}
          >
            {domain}
          </span>
        );
      }
    } catch { /* malformed URL — skip */ }
  }

  return null;
}
