import { useQueries, useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect, useMemo, useState } from "react";
import { api } from "../api/client";
import type { Column } from "../state/workspace";
import { useWorkspace } from "../state/workspace";
import { ColumnHeader } from "../components/ColumnHeader";
import { useLiveFeed } from "../state/live";
import { parseToolCallArguments } from "../components/content";
import type { SpanNode } from "../api/types";

// File-touches scenario: aggregates every `view`, `edit`, `create`, and
// `apply_patch` tool call observed in the selected session and lays the touched paths
// out as a collapsible filesystem tree.
//
// Pure frontend composition — no new backend endpoint:
//   1. /api/sessions/<cid>/span-tree → SpanNode[] (shares the
//      ["session-span-tree", session] cache with the Spans column).
//      We use this rather than /api/spans?session=&kind=execute_tool
//      because the latter only returns tool spans once their
//      tool_calls.conversation_id has been backfilled by the ancestor
//      walk — which in the Copilot flow waits for the parent chat span
//      to end. The session-span-tree endpoint is trace-scoped and
//      surfaces tool spans as soon as they land.
//   2. Walk the tree: a SpanNode whose kind_class is "execute_tool" and
//      whose name parses to view/edit/create/apply_patch is a candidate.
//   3. /api/spans/:trace_id/:span_id per matching span (cached, shared
//      with ToolDetail / ChatDetail via the ["span", trace, span]
//      query key) to read gen_ai.tool.call.arguments.path, or patch file
//      headers for apply_patch.
//
// Refreshes via the same WS feed Spans uses so newly-arrived tool calls
// fold in live.

type AccessKind = "read" | "write" | "both";

const READ_TOOLS = new Set(["view"]);
const WRITE_TOOLS = new Set(["edit", "create", "apply_patch"]);
const TRACKED_TOOLS = new Set([...READ_TOOLS, ...WRITE_TOOLS]);

interface Touch {
  path: string;
  tool: string;
  access: "read" | "write";
  span_pk: number;
  start_ns: number | null;
}

interface TreeNode {
  name: string;
  fullPath: string;        // "/" for root; otherwise leading slash preserved as-is
  children: Map<string, TreeNode>;
  reads: number;
  writes: number;
  // Touches landing exactly at this node (i.e., this node IS a file).
  fileTouches: Touch[];
}

function newNode(name: string, fullPath: string): TreeNode {
  return { name, fullPath, children: new Map(), reads: 0, writes: 0, fileTouches: [] };
}

function splitPath(p: string): { abs: boolean; parts: string[] } {
  // Normalize: collapse repeated separators, drop trailing slash.
  const abs = p.startsWith("/");
  const trimmed = p.replace(/\/+$/g, "").replace(/\/+/g, "/");
  const raw = abs ? trimmed.slice(1) : trimmed;
  const parts = raw.length === 0 ? [] : raw.split("/");
  return { abs, parts };
}

function buildTree(touches: Touch[]): TreeNode {
  const root = newNode("/", "/");
  for (const t of touches) {
    const { parts } = splitPath(t.path);
    if (parts.length === 0) continue;
    let node = root;
    let acc = "";
    for (let i = 0; i < parts.length; i++) {
      const seg = parts[i];
      acc = acc === "" ? `/${seg}` : `${acc}/${seg}`;
      let child = node.children.get(seg);
      if (!child) {
        child = newNode(seg, acc);
        node.children.set(seg, child);
      }
      node = child;
      if (t.access === "read") node.reads += 1;
      else node.writes += 1;
    }
    node.fileTouches.push(t);
  }
  // Roll up root counts so the "/" header summarizes the whole tree.
  for (const c of root.children.values()) {
    root.reads += c.reads;
    root.writes += c.writes;
  }
  return root;
}

function nodeAccess(n: TreeNode): AccessKind | null {
  if (n.reads > 0 && n.writes > 0) return "both";
  if (n.writes > 0) return "write";
  if (n.reads > 0) return "read";
  return null;
}

function accessClass(a: AccessKind | null): string {
  switch (a) {
    case "write": return "ft-write";
    case "read": return "ft-read";
    case "both": return "ft-both";
    default: return "ft-none";
  }
}

function extractApplyPatchPaths(args: unknown): string[] {
  let patchText: string | null = null;
  if (typeof args === "string") {
    patchText = args;
  } else if (args && typeof args === "object" && !Array.isArray(args)) {
    const obj = args as Record<string, unknown>;
    if (typeof obj.patch === "string") patchText = obj.patch;
    else if (typeof obj.input === "string") patchText = obj.input;
  }
  if (!patchText) return [];

  const paths = new Set<string>();
  for (const line of patchText.split(/\r?\n/)) {
    const match = line.match(
      /^\*\*\* (?:(?:Add|Update|Delete) File:|Move to:) (.+)$/
    );
    const path = match?.[1]?.trim();
    if (path) paths.add(path);
  }
  return [...paths];
}

function extractTouchPaths(toolName: string, args: unknown): string[] {
  if (toolName === "apply_patch") return extractApplyPatchPaths(args);
  if (!args || typeof args !== "object" || Array.isArray(args)) return [];
  const path = (args as Record<string, unknown>).path;
  return typeof path === "string" && path ? [path] : [];
}

export function FileTouchesScenario({ column }: { column: Column }) {
  const qc = useQueryClient();
  const columns = useWorkspace((s) => s.columns);
  const updateColumn = useWorkspace((s) => s.updateColumn);
  const session = column.config.session;

  // If this column was added after the user had already picked a session
  // elsewhere, adopt that session instead of forcing a re-select.
  useEffect(() => {
    if (session) return;
    for (const c of columns) {
      if (c.id === column.id) continue;
      if (c.config.session) {
        updateColumn(column.id, { config: { ...column.config, session: c.config.session } });
        return;
      }
    }
  }, [session, columns, column.id, column.config, updateColumn]);

  const treeQ = useQuery({
    queryKey: ["session-span-tree", session],
    queryFn: () => api.getSessionSpanTree(session!),
    enabled: !!session,
  });

  const { tick } = useLiveFeed([
    { kind: "span", entity: "span" },
    { kind: "derived", entity: "tool_call" },
  ]);
  useEffect(() => {
    if (!session) return;
    qc.invalidateQueries({ queryKey: ["session-span-tree", session] });
  }, [tick, session, qc]);

  // Walk the session span tree and pull out execute_tool spans whose
  // name parses to one of the file-touch tools. Span names emitted by
  // Copilot follow `execute_tool <tool_name>` — see
  // reference/span_hierarchy.md and reference/copilot-content.log.
  const candidateSpans = useMemo<Array<SpanNode & { tool_name: string }>>(() => {
    const out: Array<SpanNode & { tool_name: string }> = [];
    const visit = (n: SpanNode) => {
      if (n.kind_class === "execute_tool") {
        const parts = n.name.split(" ");
        if (parts.length >= 2) {
          const tool = parts.slice(1).join(" ");
          if (TRACKED_TOOLS.has(tool)) {
            out.push({ ...n, tool_name: tool });
          }
        }
      }
      for (const c of n.children) visit(c);
    };
    for (const root of treeQ.data?.tree ?? []) visit(root);
    return out;
  }, [treeQ.data]);

  // Fetch each matching span's detail. The query key matches ToolDetail /
  // ChatDetail so cache is shared.
  const detailQs = useQueries({
    queries: candidateSpans.map((s) => ({
      queryKey: ["span", s.trace_id, s.span_id],
      queryFn: () => api.getSpan(s.trace_id, s.span_id),
      enabled: true,
      staleTime: 30_000,
    })),
  });

  const touches = useMemo<Touch[]>(() => {
    const out: Touch[] = [];
    for (let i = 0; i < candidateSpans.length; i++) {
      const s = candidateSpans[i];
      const d = detailQs[i]?.data;
      if (!d) continue;
      const args = parseToolCallArguments(d.span.attributes ?? {});
      const access: "read" | "write" = WRITE_TOOLS.has(s.tool_name) ? "write" : "read";
      for (const path of extractTouchPaths(s.tool_name, args)) {
        out.push({
          path,
          tool: s.tool_name,
          access,
          span_pk: s.span_pk,
          start_ns: s.start_unix_ns,
        });
      }
    }
    return out;
  }, [candidateSpans, detailQs]);

  const tree = useMemo(() => buildTree(touches), [touches]);

  // Set of directory fullPaths currently expanded. Lifted to the
  // scenario so the header buttons can manipulate every node at once.
  // Defaults to "every directory" — the user explicitly asked for
  // expand-all on first render — and stays in sync as new directories
  // appear from incoming tool calls (we union them into the open set).
  const allDirPaths = useMemo(() => collectDirPaths(tree), [tree]);
  const [openDirs, setOpenDirs] = useState<Set<string>>(() => new Set());
  // Track which directories we have already auto-opened so the user's
  // explicit collapses survive the next live update. Newly-discovered
  // dirs default to open, previously-known dirs are left as the user
  // last set them.
  const [knownDirs, setKnownDirs] = useState<Set<string>>(() => new Set());
  useEffect(() => {
    setOpenDirs((prev) => {
      const next = new Set(prev);
      let changed = false;
      for (const p of allDirPaths) {
        if (!knownDirs.has(p)) {
          next.add(p);
          changed = true;
        }
      }
      return changed ? next : prev;
    });
    setKnownDirs((prev) => {
      if (allDirPaths.size === 0) return prev;
      const next = new Set(prev);
      let changed = false;
      for (const p of allDirPaths) {
        if (!next.has(p)) {
          next.add(p);
          changed = true;
        }
      }
      return changed ? next : prev;
    });
  }, [allDirPaths, knownDirs]);

  const toggleDir = (path: string) => {
    setOpenDirs((prev) => {
      const next = new Set(prev);
      if (next.has(path)) next.delete(path);
      else next.add(path);
      return next;
    });
  };
  const expandAll = () => setOpenDirs(new Set(allDirPaths));
  const collapseAll = () => setOpenDirs(new Set());

  const totalReads = tree.reads;
  const totalWrites = tree.writes;
  const distinctFiles = useMemo(() => {
    const set = new Set<string>();
    for (const t of touches) set.add(t.path);
    return set.size;
  }, [touches]);

  const detailsLoading = detailQs.some((q) => q.isLoading);

  return (
    <>
      <ColumnHeader column={column}>
        <span
          className="ft-warn"
          data-tip="This is a rough guess based on view/edit/create/apply_patch tool calls. It does not include file accesses performed via the bash/shell tool."
          aria-label="Approximate data warning"
        >
          [!]
        </span>
        <span className="dim">session</span>
        <span className="mono">{session ? session.slice(0, 8) : "—"}</span>
        <span className="dim">read</span>
        <span className="ft-read">{totalReads}</span>
        <span className="dim">write</span>
        <span className="ft-write">{totalWrites}</span>
        <span className="dim">{distinctFiles} file{distinctFiles === 1 ? "" : "s"}</span>
        <span className="ft-tree-btns">
          <button
            className="ft-tree-btn"
            data-tip="Expand all directories"
            data-tip-right
            aria-label="Expand all directories"
            onClick={expandAll}
            disabled={allDirPaths.size === 0}
          >
            [+]
          </button>
          <button
            className="ft-tree-btn"
            data-tip="Collapse all directories"
            data-tip-right
            aria-label="Collapse all directories"
            onClick={collapseAll}
            disabled={allDirPaths.size === 0}
          >
            [-]
          </button>
        </span>
      </ColumnHeader>
      <div className="col-body" style={{ overflow: "auto" }}>
        {!session ? (
          <div className="empty-state">Pick a session in the Live sessions column.</div>
        ) : treeQ.isLoading ? (
          <div className="empty-state">loading spans…</div>
        ) : candidateSpans.length === 0 ? (
          <div className="empty-state">no view / edit / create / apply_patch tool calls yet</div>
        ) : touches.length === 0 && detailsLoading ? (
          <div className="empty-state">loading tool args…</div>
        ) : touches.length === 0 ? (
          <div className="empty-state">no captured paths in matching tool calls</div>
        ) : (
          <FileTree root={tree} openDirs={openDirs} toggleDir={toggleDir} />
        )}
      </div>
    </>
  );
}

function collectDirPaths(root: TreeNode): Set<string> {
  const out = new Set<string>();
  const walk = (n: TreeNode) => {
    if (n.children.size === 0) return;
    out.add(n.fullPath);
    for (const c of n.children.values()) walk(c);
  };
  walk(root);
  return out;
}

function FileTree({
  root,
  openDirs,
  toggleDir,
}: {
  root: TreeNode;
  openDirs: Set<string>;
  toggleDir: (path: string) => void;
}) {
  return (
    <div className="ft-tree">
      {[...root.children.values()]
        .sort(sortNodes)
        .map((c) => (
          <NodeView
            key={c.fullPath}
            node={c}
            depth={0}
            openDirs={openDirs}
            toggleDir={toggleDir}
          />
        ))}
    </div>
  );
}

function sortNodes(a: TreeNode, b: TreeNode): number {
  // Directories first, then files; alphabetical within each group.
  const aDir = a.children.size > 0 ? 0 : 1;
  const bDir = b.children.size > 0 ? 0 : 1;
  if (aDir !== bDir) return aDir - bDir;
  return a.name.localeCompare(b.name);
}

function NodeView({
  node,
  depth,
  openDirs,
  toggleDir,
}: {
  node: TreeNode;
  depth: number;
  openDirs: Set<string>;
  toggleDir: (path: string) => void;
}) {
  const isDir = node.children.size > 0;
  const open = isDir ? openDirs.has(node.fullPath) : false;
  const access = nodeAccess(node);
  const cls = accessClass(access);

  return (
    <div className={`ft-node ${cls}`}>
      <div
        className={`ft-row${isDir ? " ft-clickable" : ""}`}
        onClick={() => isDir && toggleDir(node.fullPath)}
        title={
          node.fileTouches.length
            ? node.fileTouches
                .map((t) => `${t.tool}  (span_pk ${t.span_pk})`)
                .join("\n")
            : undefined
        }
      >
        <span className="ft-caret">{isDir ? (open ? "▾" : "▸") : "·"}</span>
        <span className="ft-name mono">{node.name}{isDir ? "/" : ""}</span>
        <span className="ft-counts">
          {node.reads > 0 && <span className="ft-read" title="reads">r{node.reads}</span>}
          {node.writes > 0 && <span className="ft-write" title="writes">w{node.writes}</span>}
        </span>
      </div>
      {isDir && open && (
        <div className="ft-children">
          {[...node.children.values()].sort(sortNodes).map((c) => (
            <NodeView
              key={c.fullPath}
              node={c}
              depth={depth + 1}
              openDirs={openDirs}
              toggleDir={toggleDir}
            />
          ))}
        </div>
      )}
    </div>
  );
}
