import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useVirtualizer } from "@tanstack/react-virtual";
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

// Locate the chat span that consumes a given tool span's response.
//
// OTel GenAI semconv permits two valid hierarchy shapes for tool spans
// and the two producers we observe instrument them differently. Each
// shape implies a different "response-consuming chat":
//
//   * Sibling shape (Copilot CLI, agent-loop-level instrumentation):
//     the tool span is a sibling of chat spans under a shared parent.
//     The tool is dispatched mid-stream from a still-running chat span;
//     its response is appended to that same chat span's input.messages
//     before the chat span ends. Find the chat sibling whose
//     [start, end] temporally ENCLOSES the tool's [start, end].
//
//   * Nested shape (opencode, SDK-level instrumentation): the tool span
//     is a descendant of a chat span ("parent chat"). The tool runs
//     during the parent chat's lifetime but its response is NOT in the
//     parent chat's input — it surfaces in the input of the NEXT chat
//     span chronologically (the one that fires after the parent chat
//     completes and the loop iterates). Find the chat span anywhere in
//     the tree whose sortKey is the smallest value strictly greater
//     than the parent chat's sortKey.
//
// Shape-driven, not vendor-gated: dispatched on whether the tool has a
// chat ancestor. Falls back to next chat sibling if neither rule fires
// (e.g. malformed tree). Service_name is not consulted.
function findFollowingChatSpanId(tree: SpanNode[], span_id: string): string | undefined {
  // Walk the tree, capturing the picked node and its ancestor chain.
  let picked: SpanNode | null = null;
  let pickedAncestors: SpanNode[] = [];
  const find = (nodes: SpanNode[], ancestors: SpanNode[]): boolean => {
    for (const n of nodes) {
      if (n.span_id === span_id) {
        picked = n;
        pickedAncestors = ancestors;
        return true;
      }
      if (find(n.children ?? [], [...ancestors, n])) return true;
    }
    return false;
  };
  find(tree, []);
  if (!picked) return undefined;
  const pickedNode: SpanNode = picked;

  // Nested shape: nearest chat ancestor exists → next chat globally.
  for (let i = pickedAncestors.length - 1; i >= 0; i--) {
    if (pickedAncestors[i].kind_class === "chat") {
      const threshold = sortKey(pickedAncestors[i]);
      const thresholdPk = pickedAncestors[i].span_pk ?? 0;
      let best: SpanNode | null = null;
      let bestKey = Number.POSITIVE_INFINITY;
      const walk = (nodes: SpanNode[]) => {
        for (const n of nodes) {
          if (n.kind_class === "chat") {
            const k = sortKey(n);
            const isAfter =
              k > threshold ||
              (k === threshold && (n.span_pk ?? 0) > thresholdPk);
            if (isAfter && k < bestKey) {
              best = n;
              bestKey = k;
            }
          }
          walk(n.children ?? []);
        }
      };
      walk(tree);
      return (best as SpanNode | null)?.span_id;
    }
  }

  // Sibling shape: chat sibling whose lifetime encloses the tool.
  const siblingsHit = findSiblings(tree, span_id);
  if (siblingsHit) {
    const tStart = pickedNode.start_unix_ns ?? 0;
    const tEnd = pickedNode.end_unix_ns ?? tStart;
    for (const sib of siblingsHit.siblings) {
      if (sib.kind_class !== "chat") continue;
      const sStart = sib.start_unix_ns ?? 0;
      const sEnd = sib.end_unix_ns ?? Number.POSITIVE_INFINITY;
      if (sStart <= tStart && sEnd >= tEnd) return sib.span_id;
    }
  }

  // Fallback: next chat sibling chronologically.
  return findNextChatSiblingId(tree, span_id);
}

// Find the most recent (by sortKey) chat span that is a descendant of
// the given node. Used to route invoke_agent selections to ChatDetail.
function findLatestChatDescendant(node: SpanNode): SpanNode | undefined {
  let best: SpanNode | undefined;
  let bestKey = -1;
  const walk = (nodes: SpanNode[]) => {
    for (const n of nodes) {
      if (n.kind_class === "chat") {
        const k = sortKey(n);
        if (k > bestKey) { bestKey = k; best = n; }
      }
      walk(n.children ?? []);
    }
  };
  walk(node.children ?? []);
  return best;
}

interface FlatRow {
  node: SpanNode;
  depth: number;
  chatAncestorPk: number | null;
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

function flattenSpanTreeWithMeta(tree: SpanNode[], collapsed?: Set<string>): FlatRow[] {
  const rows: FlatRow[] = [];
  const walk = (nodes: SpanNode[], depth: number, chatAncestorPk: number | null) => {
    for (const n of nodes) {
      const myChat = n.kind_class === "chat" ? n.span_pk : chatAncestorPk;
      rows.push({ node: n, depth, chatAncestorPk: myChat });
      if (collapsed?.has(n.span_id)) continue;
      walk(n.children ?? [], depth + 1, myChat);
    }
  };
  walk(tree, 0, null);
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

  // --- batch arrival smoothing -----------------------------------------
  // Spans arrive in batches via the live feed, often taller than the
  // viewport. Revealing each new batch all at once makes the actual
  // ingestion rate impossible to perceive in follow mode. Instead, scale
  // each new span's reveal time by its `start_unix_ns` offset within the
  // batch to a 2s window (best-effort) and clamp consecutive reveals to
  // ≥ 1000/60 ≈ 16.67ms apart (hard 60/sec cap — large batches extend
  // past 2s). The first batch on a fresh session reveals immediately so
  // historical backfill isn't gated by the animation.
  const SMOOTH_WINDOW_MS = 2000;
  const SMOOTH_MIN_GAP_MS = 1000 / 60;
  const revealedIdsRef = useRef<Set<string>>(new Set());
  const queueRef = useRef<Array<{ id: string; at: number }>>([]);
  const timerRef = useRef<number | null>(null);
  const animationSessionRef = useRef<string | undefined>(undefined);
  const [revealVersion, setRevealVersion] = useState(0);

  const drainQueue = useCallback(() => {
    timerRef.current = null;
    if (queueRef.current.length === 0) return;
    const now = Date.now();
    let revealed = false;
    while (queueRef.current.length > 0 && queueRef.current[0].at <= now) {
      const item = queueRef.current.shift()!;
      revealedIdsRef.current.add(item.id);
      revealed = true;
    }
    if (revealed) setRevealVersion((v) => v + 1);
    if (queueRef.current.length > 0) {
      const delay = Math.max(0, queueRef.current[0].at - Date.now());
      timerRef.current = window.setTimeout(drainQueue, delay);
    }
  }, []);

  useEffect(() => {
    // Synchronously reset all smoothing state on session change so the
    // new session's first tree update is treated as a fresh first-load.
    if (animationSessionRef.current !== session) {
      animationSessionRef.current = session;
      revealedIdsRef.current = new Set();
      queueRef.current = [];
      if (timerRef.current != null) {
        clearTimeout(timerRef.current);
        timerRef.current = null;
      }
      setRevealVersion((v) => v + 1);
    }

    if (tree.length === 0) return;

    // Find spans in the new tree that we haven't seen yet (preorder DFS
    // so parents precede children in the queue — combined with the
    // monotonic gap clamp this guarantees a parent reveals before its
    // children).
    const known = new Set<string>(revealedIdsRef.current);
    for (const q of queueRef.current) known.add(q.id);
    const fresh: SpanNode[] = [];
    const walkFresh = (nodes: SpanNode[]) => {
      for (const n of nodes) {
        if (!known.has(n.span_id)) fresh.push(n);
        walkFresh(n.children ?? []);
      }
    };
    walkFresh(tree);
    if (fresh.length === 0) return;

    // First batch on this session: reveal everything immediately so we
    // don't animate historical backfill.
    if (revealedIdsRef.current.size === 0 && queueRef.current.length === 0) {
      for (const n of fresh) revealedIdsRef.current.add(n.span_id);
      setRevealVersion((v) => v + 1);
      return;
    }

    // The list is rendered newest→oldest (top→bottom), so reveal the
    // newest span in the batch first and progressively older ones
    // beneath it: offset 0 for max(ts), SMOOTH_WINDOW_MS for min(ts).
    // The intra-batch density still tracks the actual rate, just
    // mirrored in time to match the list orientation.
    const tsOf = (n: SpanNode) => n.start_unix_ns ?? n.end_unix_ns ?? 0;
    const tsList = fresh.map(tsOf).filter((t) => t > 0);
    const minTs = tsList.length > 0 ? Math.min(...tsList) : 0;
    const maxTs = tsList.length > 0 ? Math.max(...tsList) : 0;
    const range = maxTs - minTs;
    const now = Date.now();
    const atMap = new Map<string, number>();
    for (const n of fresh) {
      const t = tsOf(n);
      const scaled =
        range > 0 && t >= minTs ? ((maxTs - t) / range) * SMOOTH_WINDOW_MS : 0;
      atMap.set(n.span_id, now + scaled);
    }
    // Hierarchy clamp: if a parent and any of its descendants are both
    // in this batch, the parent must reveal no later than its earliest
    // descendant. Otherwise the parent's later reveal time would keep
    // the descendant hidden by the filter, collapsing the intended
    // cadence into a single simultaneous appearance.
    const freshIds = new Set(atMap.keys());
    const postOrder = (nodes: SpanNode[]): void => {
      for (const n of nodes) {
        postOrder(n.children ?? []);
        if (!freshIds.has(n.span_id)) continue;
        let earliest = atMap.get(n.span_id)!;
        for (const c of n.children ?? []) {
          const ca = atMap.get(c.span_id);
          if (ca != null && ca < earliest) earliest = ca;
        }
        atMap.set(n.span_id, earliest);
      }
    };
    postOrder(tree);
    for (const [id, at] of atMap) {
      queueRef.current.push({ id, at });
    }

    // Re-normalize the merged queue: sort by `at`, then clamp each entry
    // to be at least SMOOTH_MIN_GAP_MS after the previous one so the
    // global reveal rate never exceeds 60/sec across overlapping
    // batches.
    queueRef.current.sort((a, b) => a.at - b.at);
    for (let i = 1; i < queueRef.current.length; i++) {
      const prev = queueRef.current[i - 1].at;
      if (queueRef.current[i].at < prev + SMOOTH_MIN_GAP_MS) {
        queueRef.current[i].at = prev + SMOOTH_MIN_GAP_MS;
      }
    }

    if (timerRef.current == null) {
      const delay = Math.max(0, queueRef.current[0].at - Date.now());
      timerRef.current = window.setTimeout(drainQueue, delay);
    }
  }, [tree, session, drainQueue]);

  useEffect(() => () => {
    if (timerRef.current != null) clearTimeout(timerRef.current);
  }, []);

  // Tree with un-revealed spans pruned. Downstream consumers (nodeMap,
  // latestToolSpan, SpanTreeView, follow-mode auto-advance, etc.) all
  // operate on this so the visual cadence drives every behavior.
  // `revealVersion` is the render-invalidation signal for mutations to
  // `revealedIdsRef`.
  const displayedTree = useMemo(() => {
    const revealed = revealedIdsRef.current;
    if (revealed.size === 0) return [];
    const filter = (nodes: SpanNode[]): SpanNode[] => {
      const out: SpanNode[] = [];
      for (const n of nodes) {
        if (!revealed.has(n.span_id)) continue;
        const kids = filter(n.children ?? []);
        out.push(kids.length === (n.children?.length ?? 0) ? n : { ...n, children: kids });
      }
      return out;
    };
    return filter(tree);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [tree, revealVersion]);

  // O(1) span lookup by ID — single walk shared by follow-mode, search, etc.
  const nodeMap = useMemo(() => {
    const m = new Map<string, SpanNode>();
    const walk = (nodes: SpanNode[]) => {
      for (const n of nodes) {
        m.set(n.span_id, n);
        walk(n.children ?? []);
      }
    };
    walk(displayedTree);
    return m;
  }, [displayedTree]);

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
    // Auto-sync follow mode on user-initiated selections.
    if (isAutoAdvancing.current) {
      isAutoAdvancing.current = false;
    } else {
      // User-initiated: engage follow if they picked the latest tool span.
      setFollowMode(
        latestToolSpan != null && span_id === latestToolSpan.span_id
      );
    }

    // For execute_tool selections, also auto-advance chat_detail
    // columns to the chat span that consumes this tool's response.
    // Shape-aware: opencode nests tool spans under chat (response
    // lands in the NEXT chat globally); Copilot has tool spans as
    // siblings of chat spans (response lands in the chat sibling that
    // temporally encloses the tool). See findFollowingChatSpanId.
    let followingChatSpanId: string | undefined;
    let toolCallId: string | undefined;
    if (kind_class === "execute_tool" && displayedTree.length > 0) {
      const hit = findSiblings(displayedTree, span_id);
      toolCallId = hit?.picked.projection.tool_call?.call_id ?? undefined;
      followingChatSpanId = findFollowingChatSpanId(displayedTree, span_id);
    }

    // For invoke_agent selections, advance chat_detail to the most
    // recent chat span under the agent so the user immediately sees
    // the sub-agent's conversation.
    let agentChatSpanId: string | undefined;
    if (kind_class === "invoke_agent") {
      const agentNode = nodeMap.get(span_id);
      if (agentNode) {
        const latestChat = findLatestChatDescendant(agentNode);
        if (latestChat) agentChatSpanId = latestChat.span_id;
      }
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
      if (c.scenarioType === "chat_detail") {
        const chatTarget = followingChatSpanId ?? agentChatSpanId;
        if (chatTarget) {
          updateColumn(c.id, {
            config: {
              ...c.config,
              selected_trace_id: trace_id,
              selected_span_id: chatTarget,
              selected_tool_call_id: toolCallId,
            },
          });
        }
      }
    });
  };

  const onPickTrace = (trace_id: string, span_id?: string, kind_class?: KindClass) => {
    if (!span_id) return;
    onPickSpan(trace_id, span_id, kind_class ?? "other");
  };

  // --- follow mode (explicit state) ---
  // When enabled, auto-advances selection to the latest tool span as
  // new spans arrive. Enabled automatically when the user selects the
  // latest tool span; disabled when they select something else.
  const TOOL_KINDS: KindClass[] = ["execute_tool", "external_tool"];
  const [followMode, setFollowMode] = useState(false);
  const isAutoAdvancing = useRef(false);

  const latestToolSpan = useMemo(() => {
    let best: SpanNode | null = null;
    let bestKey = -1;
    const walk = (nodes: SpanNode[]) => {
      for (const n of nodes) {
        if (TOOL_KINDS.includes(n.kind_class)) {
          const k = sortKey(n);
          if (k > bestKey) { bestKey = k; best = n; }
        }
        walk(n.children ?? []);
      }
    };
    walk(displayedTree);
    return best as SpanNode | null;
  }, [displayedTree]);

  useEffect(() => {
    if (!followMode || !latestToolSpan) return;
    if (latestToolSpan.span_id !== selected_span_id) {
      isAutoAdvancing.current = true;
      onPickSpan(latestToolSpan.trace_id, latestToolSpan.span_id, latestToolSpan.kind_class);
    }
    // onPickSpan is intentionally omitted — it closes over `columns`
    // and recreates each render; we only care about advances driven by
    // tree updates and selection changes.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [followMode, latestToolSpan, selected_span_id]);

  // Latest chat span by sortKey across the revealed tree. Mirrors
  // latestToolSpan; drives follow-mode advancement of ChatDetail columns.
  const latestChatSpan = useMemo(() => {
    let best: SpanNode | null = null;
    let bestKey = -1;
    const walk = (nodes: SpanNode[]) => {
      for (const n of nodes) {
        if (n.kind_class === "chat") {
          const k = sortKey(n);
          if (k > bestKey) { bestKey = k; best = n; }
        }
        walk(n.children ?? []);
      }
    };
    walk(displayedTree);
    return best as SpanNode | null;
  }, [displayedTree]);

  // Follow-mode chat catch-up. ChatDetail cannot be advanced at the moment
  // follow jumps to a new tool span, because the chat span that consumes the
  // tool's response usually hasn't arrived yet — and several batched tool
  // calls may all land in a single chat span, so there's no reliable 1:1
  // tool→chat target to resolve up front. Instead, while following, advance
  // ChatDetail to the latest chat span as soon as it lands, pointing its
  // tool-call arrow at whichever tool ToolDetail is currently showing. The
  // converge-only guard makes this idempotent and leaves a user's manual chat
  // pick in place until the next chat span arrives.
  useEffect(() => {
    if (!followMode || !latestChatSpan) return;
    const chatSpan = latestChatSpan;
    const toolCallId = latestToolSpan?.projection.tool_call?.call_id ?? undefined;
    columns.forEach((c) => {
      if (c.scenarioType !== "chat_detail") return;
      if (
        c.config.selected_span_id === chatSpan.span_id &&
        c.config.selected_tool_call_id === toolCallId
      ) return;
      updateColumn(c.id, {
        config: {
          ...c.config,
          selected_trace_id: chatSpan.trace_id,
          selected_span_id: chatSpan.span_id,
          selected_tool_call_id: toolCallId,
        },
      });
    });
  }, [followMode, latestChatSpan, latestToolSpan, columns, updateColumn]);

  // --- collapse state (lifted from SpanTreeView for header buttons) ---
  const [userCollapsed, setUserCollapsed] = useState<Set<string>>(new Set());
  useEffect(() => { setUserCollapsed(new Set()); }, [session]);

  const collapseAll = useCallback(() => {
    // Collapse every span that has children.
    const ids = new Set<string>();
    const walk = (nodes: SpanNode[]) => {
      for (const n of nodes) {
        if (n.children && n.children.length > 0) ids.add(n.span_id);
        walk(n.children ?? []);
      }
    };
    walk(displayedTree);
    setUserCollapsed(ids);
  }, [displayedTree]);

  const expandAll = useCallback(() => {
    setUserCollapsed(new Set());
  }, []);

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
        <div style={{ display: "grid", gridTemplateColumns: "auto 1fr", gap: "4px", width: "100%", alignItems: "center" }}>
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
          <div style={{ display: "flex", gap: 4, alignItems: "center" }}>
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
                style={{ minWidth: 80, flex: "1 1 auto" }}
              />
            )}
            <label
              title="Auto-follow the latest tool span"
              style={{ display: "inline-flex", alignItems: "center", gap: 2, cursor: "pointer", whiteSpace: "nowrap" }}
            >
              follow
              <input
                type="checkbox"
                checked={followMode}
                onChange={(e) => {
                  const on = e.target.checked;
                  setFollowMode(on);
                  if (on && latestToolSpan && latestToolSpan.span_id !== selected_span_id) {
                    isAutoAdvancing.current = true;
                    onPickSpan(latestToolSpan.trace_id, latestToolSpan.span_id, latestToolSpan.kind_class);
                  }
                }}
                style={{ position: "absolute", opacity: 0, width: 0, height: 0 }}
              />
              <span style={{
                display: "inline-flex",
                alignItems: "center",
                justifyContent: "center",
                width: "1.8em",
                height: "1.8em",
                border: "1px solid var(--border)",
                borderRadius: 2,
                background: "var(--bg-1)",
                color: followMode ? "#fff" : "transparent",
                fontSize: "1.1em",
                cursor: "pointer",
              }}>
                <span style={{ fontSize: "1.3em", lineHeight: 1 }}>✓</span>
              </span>
            </label>
            <button
              title="Collapse all"
              aria-label="Collapse all"
              onClick={collapseAll}
              disabled={!session}
              style={{ padding: 0, fontSize: "1.1em", borderRadius: 0, width: "1.8em", height: "1.8em" }}
            >
              −
            </button>
            <button
              title="Expand all"
              aria-label="Expand all"
              onClick={expandAll}
              disabled={!session}
              style={{ padding: 0, fontSize: "1.1em", borderRadius: 0, width: "1.8em", height: "1.8em" }}
            >
              +
            </button>
          </div>
        </div>
      </ColumnHeader>
      <div className="col-body list" style={{ overflow: session ? "hidden" : "auto" }}>
        {session ? (
          <SpanTreeView
            key={session}
            tree={displayedTree}
            loading={sessionTreeQ.isLoading}
            kindFilter={kind_filter}
            selectedSpanId={selected_span_id}
            onSelect={onPickSpan}
            searchHitSpanIds={searchHitSpanIds}
            userCollapsed={userCollapsed}
            setUserCollapsed={setUserCollapsed}
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

// --- per-trace tree (virtualized) ----------------------------------------

const ROW_HEIGHT_PX = 24;

function SpanTreeView({
  tree,
  loading,
  kindFilter,
  selectedSpanId,
  onSelect,
  searchHitSpanIds,
  userCollapsed,
  setUserCollapsed,
}: {
  tree: SpanNode[];
  loading: boolean;
  kindFilter: KindClass | undefined;
  selectedSpanId: string | undefined;
  onSelect: (t: string, s: string, k: KindClass) => void;
  searchHitSpanIds: Set<string> | null;
  userCollapsed: Set<string>;
  setUserCollapsed: React.Dispatch<React.SetStateAction<Set<string>>>;
}) {

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
    () => flattenSpanTreeWithMeta(tree, effectiveCollapsed),
    [tree, effectiveCollapsed],
  );

  // Scroll container ref for the virtualizer.
  const scrollRef = useRef<HTMLDivElement>(null);

  const virtualizer = useVirtualizer({
    count: rows.length,
    getScrollElement: () => scrollRef.current,
    estimateSize: () => ROW_HEIGHT_PX,
    overscan: 15,
  });

  // Scroll selected row into view when selection changes.
  useEffect(() => {
    if (!selectedSpanId) return;
    const idx = rows.findIndex((r) => r.node.span_id === selectedSpanId);
    if (idx >= 0) virtualizer.scrollToIndex(idx, { align: "auto" });
    // virtualizer instance is stable across renders
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [selectedSpanId, rows]);

  const onKeyDown = (e: KeyboardEvent<HTMLDivElement>) => {
    if (e.key !== "ArrowDown" && e.key !== "ArrowUp") return;
    e.preventDefault();
    if (rows.length === 0) return;

    const current = rows.findIndex((r) => r.node.span_id === selectedSpanId);
    const nextIndex =
      e.key === "ArrowDown"
        ? current < 0
          ? 0
          : Math.min(current + 1, rows.length - 1)
        : current < 0
          ? rows.length - 1
          : Math.max(current - 1, 0);
    const next = rows[nextIndex];
    if (!next || next.node.span_id === selectedSpanId) return;
    onSelect(next.node.trace_id, next.node.span_id, next.node.kind_class);
  };

  if (loading) return <div className="empty-state">loading…</div>;
  if (tree.length === 0) return <div className="empty-state">no spans in trace</div>;

  const virtualItems = virtualizer.getVirtualItems();

  return (
    <div
      ref={scrollRef}
      tabIndex={0}
      onMouseDown={(e) => e.currentTarget.focus({ preventScroll: true })}
      onKeyDown={onKeyDown}
      style={{ height: "100%", overflow: "auto" }}
    >
      <div style={{ height: virtualizer.getTotalSize(), position: "relative" }}>
        {virtualItems.map((vi) => {
          const { node, depth, chatAncestorPk } = rows[vi.index];
          return (
            <SpanTreeRow
              key={node.span_pk}
              node={node}
              depth={depth}
              chatAncestorPk={chatAncestorPk}
              kindFilter={kindFilter}
              selectedSpanId={selectedSpanId}
              onSelect={onSelect}
              searchHitSpanIds={searchHitSpanIds}
              effectiveCollapsed={effectiveCollapsed}
              toggleCollapse={toggleCollapse}
              style={{
                position: "absolute",
                top: 0,
                left: 0,
                width: "100%",
                transform: `translateY(${vi.start}px)`,
              }}
            />
          );
        })}
      </div>
    </div>
  );
}

function SpanTreeRow({
  node,
  depth,
  kindFilter,
  selectedSpanId,
  onSelect,
  chatAncestorPk,
  searchHitSpanIds,
  effectiveCollapsed,
  toggleCollapse,
  style,
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
  style?: React.CSSProperties;
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
  const hasChildren = node.children.length > 0;
  const collapsed = effectiveCollapsed.has(node.span_id);

  // Clear hover state if this row unmounts while hovered (virtualization).
  const hoveredRef = useRef(false);
  useEffect(() => {
    return () => { if (hoveredRef.current) setHoveredChatPk(null); };
  }, [setHoveredChatPk]);

  return (
    <div
      className={`row${sel ? " sel" : ""}${isSearchHit ? " search-hit" : ""}${searchMiss ? " search-miss" : ""}${dim ? " kind-dim" : ""}`}
      style={{ ...style, height: ROW_HEIGHT_PX, overflow: "hidden", paddingLeft: depth * 12 + 6 }}
      onClick={() => onSelect(node.trace_id, node.span_id, node.kind_class)}
      onMouseEnter={() => { hoveredRef.current = true; setHoveredChatPk(hoverChatPk ?? null); }}
      onMouseLeave={() => { hoveredRef.current = false; setHoveredChatPk(null); }}
    >
      <span className={kindCls(node.kind_class)}>{kindLabel(node.kind_class)}</span>
      {node.ingestion_state === "placeholder" && (
        <span className="tag warn"><RollingDots /></span>
      )}
      <ProjectionChips projection={node.projection} />
      {(node.projection?.tool_call?.tool_name === "bash" || node.projection?.tool_call?.tool_name === "powershell") && (
        <BashCommandChip trace_id={node.trace_id} span_id={node.span_id} />
      )}
      {node.projection?.tool_call?.tool_name === "skill" && (
        <SkillNameChip trace_id={node.trace_id} span_id={node.span_id} />
      )}
      <TargetBadge trace_id={node.trace_id} span_id={node.span_id} />
      {(node.projection?.tool_call?.tool_name === "edit" ||
        node.projection?.tool_call?.tool_name === "create" ||
        node.projection?.tool_call?.tool_name === "apply_patch" ||
        node.projection?.tool_call?.tool_name === "write") && (
        <DiffStatBadge
          trace_id={node.trace_id}
          span_id={node.span_id}
          tool_name={node.projection.tool_call.tool_name}
        />
      )}
      <ReportIntentTitle nodes={node.children} />
      {node.projection?.tool_call && (
        <DescriptionLabel trace_id={node.trace_id} span_id={node.span_id} />
      )}
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
          style={collapsed ? { display: "inline-block", transform: "rotate(90deg)" } : undefined}
        >
          ▾
        </button>
      ) : (
        <span className="row-action" style={{ visibility: "hidden" }}>▾</span>
      )}
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

  // Try "path" (Copilot) or "filePath" (opencode) → show basename
  const pathRaw = rec.path ?? rec.filePath;
  if (typeof pathRaw === "string" && pathRaw.length > 0) {
    // Windows paths start with a drive letter (e.g. C:\); split on \ only there.
    // On Unix, \ is an escape character in paths (e.g. my\ file.txt), not a separator.
    const isWindows = /^[a-zA-Z]:[\\\/]/.test(pathRaw);
    const fileName = pathRaw.split(isWindows ? /[\\/]/ : "/").pop() ?? pathRaw;
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

// Renders the tool call's `description` argument (when present) as an
// unadorned white inline label appended at the end of the row's
// variable-width content. Mirrors ReportIntentText's lightweight chrome
// (no border/background) and shares the same `["span", trace_id,
// span_id]` query cache as the surrounding chips.
function DescriptionLabel({
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
  const desc = (args as Record<string, unknown>).description;
  if (typeof desc !== "string" || !desc) return null;
  return <span style={{ marginLeft: 6, color: "#fff" }}>{desc}</span>;
}

// Count newline-terminated lines in a string. A trailing newline is
// treated as a line terminator (not a separator) so "foo\n" counts as 1
// line, matching how diffs report changes.
function countLines(s: string): number {
  if (s.length === 0) return 0;
  const trimmed = s.endsWith("\n") ? s.slice(0, -1) : s;
  return trimmed.split("\n").length;
}

// Parse a unified-diff patch body and return its added/removed line
// counts. Skips `+++`/`---` file headers; everything else starting with
// `+` or `-` is counted.
function countPatchLines(patchText: string): { added: number; removed: number } {
  let added = 0;
  let removed = 0;
  for (const line of patchText.split(/\r?\n/)) {
    if (line.startsWith("+++") || line.startsWith("---")) continue;
    if (line.startsWith("+")) added++;
    else if (line.startsWith("-")) removed++;
  }
  return { added, removed };
}

// Renders red (-N) and green (+M) line-change badges next to the file
// name on file-mutating tool spans (`edit`, `create`, `write`,
// `apply_patch`). Reuses the same `["span", trace_id, span_id]` query
// cache as TargetBadge / BashCommandChip / FileTouches / ToolDetail so
// it is free of extra requests once any of those siblings has loaded.
function DiffStatBadge({
  trace_id,
  span_id,
  tool_name,
}: {
  trace_id: string;
  span_id: string;
  tool_name: string;
}) {
  const q = useQuery({
    queryKey: ["span", trace_id, span_id],
    queryFn: () => api.getSpan(trace_id, span_id),
    enabled: !!trace_id && !!span_id,
    staleTime: 30_000,
  });
  if (!q.data) return null;
  const args = parseToolCallArguments(q.data.span.attributes ?? {});

  let added = 0;
  let removed = 0;
  if (tool_name === "apply_patch") {
    // FileTouches' extractApplyPatchPaths confirms apply_patch carries
    // the patch body in either `patch` or `input` keys, or — rarely —
    // as the raw string args. Handle the string form first so we don't
    // exclude it via the object-shape guard below.
    let patchText = "";
    if (typeof args === "string") {
      patchText = args;
    } else if (args && typeof args === "object" && !Array.isArray(args)) {
      const obj = args as Record<string, unknown>;
      if (typeof obj.patch === "string") patchText = obj.patch;
      else if (typeof obj.input === "string") patchText = obj.input;
    }
    const counts = countPatchLines(patchText);
    added = counts.added;
    removed = counts.removed;
  } else {
    if (!args || typeof args !== "object" || Array.isArray(args)) return null;
    const rec = args as Record<string, unknown>;
    if (tool_name === "edit") {
      // ToolDetail's EditArgs proves the shape: edit replaces an old
      // snippet with a new snippet within the target file. Each is a
      // verbatim multi-line string, so line counts give the natural
      // diff stat. Copilot uses `old_str`/`new_str`; opencode uses
      // `oldString`/`newString`.
      const oldStr =
        typeof rec.old_str === "string"
          ? rec.old_str
          : typeof rec.oldString === "string"
            ? rec.oldString
            : "";
      const newStr =
        typeof rec.new_str === "string"
          ? rec.new_str
          : typeof rec.newString === "string"
            ? rec.newString
            : "";
      removed = countLines(oldStr);
      added = countLines(newStr);
    } else if (tool_name === "create" || tool_name === "write") {
      // `create` (Copilot) and `write` (opencode) both write a
      // brand-new file. Body field name varies: `file_text` (legacy
      // Copilot), `content` (newer Copilot / opencode). All lines are
      // additions.
      const text =
        typeof rec.file_text === "string"
          ? rec.file_text
          : typeof rec.content === "string"
            ? rec.content
            : "";
      added = countLines(text);
    } else {
      return null;
    }
  }

  if (added === 0 && removed === 0) return null;

  return (
    <>
      {removed > 0 && (
        <span className="tag ib-badge-removed" style={{ marginRight: 4 }}>
          -{removed}
        </span>
      )}
      {added > 0 && (
        <span className="tag ib-badge-added" style={{ marginRight: 4 }}>
          +{added}
        </span>
      )}
    </>
  );
}
