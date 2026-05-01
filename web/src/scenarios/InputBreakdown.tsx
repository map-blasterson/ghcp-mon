import { useMemo, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { api } from "../api/client";
import type { Column } from "../state/workspace";
import { ColumnHeader } from "../components/ColumnHeader";
import { KindBadge } from "../components/KindBadge";
import {
  NO_CONTENT_LINE,
  parseInputMessages,
  parseOutputMessages,
  parseSystemInstructions,
  hasCapturedContent,
  type Message,
  type Part,
} from "../components/content";

// ---- Tree model ----

type IBType =
  | "root"
  | "system"
  | "system_part"
  | "tool_def_root"
  | "tool_def"
  | "input_messages_root"
  | "output_messages_root"
  | "message_user"
  | "message_assistant"
  | "message_tool"
  | "message_system"
  | "message_unknown"
  | "text_part"
  | "reasoning_part"
  | "tool_call_part"
  | "tool_call_response_part"
  | "unknown_part"
  | "json_object"
  | "json_array";

interface Node {
  id: string;
  type: IBType;
  label: string;
  bytes: number;
  children: Node[];
  primitive?: { key: string; value: string }[];
  meta?: string;
}

function safeBytes(v: unknown): number {
  try {
    return JSON.stringify(v ?? null).length;
  } catch {
    return 0;
  }
}

function fmtKB(bytes: number): string {
  const kb = bytes / 1024;
  if (kb < 0.1) return "<0.1 KB";
  return `${kb.toFixed(1)} KB`;
}

function isPlainObject(v: unknown): v is Record<string, unknown> {
  return !!v && typeof v === "object" && !Array.isArray(v);
}

function parseToolDefinitions(a: Record<string, unknown>): unknown[] {
  const raw = a["gen_ai.tool.definitions"];
  if (raw == null) return [];
  if (Array.isArray(raw)) return raw;
  if (typeof raw === "string") {
    try {
      const v = JSON.parse(raw);
      return Array.isArray(v) ? v : [];
    } catch {
      return [];
    }
  }
  return [];
}

function maybeParseJson(v: unknown): unknown {
  if (typeof v !== "string") return v;
  const s = v.trim();
  if (!s.startsWith("{") && !s.startsWith("[")) return v;
  try {
    return JSON.parse(s);
  } catch {
    return v;
  }
}

function jsonNode(value: unknown, idPath: string, label: string): Node | null {
  if (Array.isArray(value)) {
    const children: Node[] = [];
    const primitive: { key: string; value: string }[] = [];
    for (let i = 0; i < value.length; i++) {
      const child = value[i];
      const cid = `${idPath}/${i}`;
      if (Array.isArray(child) || isPlainObject(child)) {
        const n = jsonNode(child, cid, `[${i}]`);
        if (n) children.push(n);
      } else {
        primitive.push({ key: `[${i}]`, value: stringifyPrim(child) });
      }
    }
    return {
      id: idPath, type: "json_array", label, bytes: safeBytes(value),
      children, primitive: primitive.length ? primitive : undefined,
      meta: `array · ${value.length}`,
    };
  }
  if (isPlainObject(value)) {
    const children: Node[] = [];
    const primitive: { key: string; value: string }[] = [];
    for (const [k, v] of Object.entries(value)) {
      const cid = `${idPath}/${encodeKey(k)}`;
      if (Array.isArray(v) || isPlainObject(v)) {
        const n = jsonNode(v, cid, k);
        if (n) children.push(n);
      } else {
        primitive.push({ key: k, value: stringifyPrim(v) });
      }
    }
    return {
      id: idPath, type: "json_object", label, bytes: safeBytes(value),
      children, primitive: primitive.length ? primitive : undefined,
      meta: `object · ${Object.keys(value).length} key${Object.keys(value).length === 1 ? "" : "s"}`,
    };
  }
  return null;
}

function encodeKey(k: string): string { return k.replace(/%/g, "%25").replace(/\//g, "%2F"); }

function stringifyPrim(v: unknown): string {
  if (v === null) return "null";
  if (typeof v === "string") return JSON.stringify(v);
  return String(v);
}

function partTypeToIb(t: string): IBType {
  switch (t) {
    case "text": return "text_part";
    case "reasoning": return "reasoning_part";
    case "tool_call": return "tool_call_part";
    case "tool_call_response": return "tool_call_response_part";
    default: return "unknown_part";
  }
}

function roleToIb(role: string): IBType {
  switch (role) {
    case "user": return "message_user";
    case "assistant": return "message_assistant";
    case "tool": return "message_tool";
    case "system": return "message_system";
    default: return "message_unknown";
  }
}

function buildPartNode(part: Part, idPath: string): Node {
  const t = typeof part.type === "string" ? part.type : "unknown";
  const ibType = partTypeToIb(t);
  const bytes = safeBytes(part);

  if (t === "text" || t === "reasoning") {
    const content = (part as { content?: string }).content ?? "";
    const preview = content.length > 80 ? content.slice(0, 80) + "…" : content;
    return {
      id: idPath, type: ibType, label: t, bytes, children: [],
      primitive: [{ key: "content", value: stringifyPrim(content) }],
      meta: `${content.length} ch · "${preview.replace(/\n/g, " ")}"`,
    };
  }

  if (t === "tool_call") {
    const tc = part as { id?: string; name?: string; arguments?: unknown };
    const argsNode = (Array.isArray(tc.arguments) || isPlainObject(tc.arguments))
      ? jsonNode(tc.arguments, `${idPath}/arguments`, "arguments")
      : null;
    const primitive: { key: string; value: string }[] = [
      { key: "name", value: stringifyPrim(tc.name ?? "") },
      { key: "id", value: stringifyPrim(shortenId(tc.id ?? "")) },
    ];
    if (!argsNode && tc.arguments !== undefined) {
      primitive.push({ key: "arguments", value: stringifyPrim(tc.arguments) });
    }
    return {
      id: idPath, type: "tool_call_part", label: "tool_call", bytes,
      children: argsNode ? [argsNode] : [], primitive, meta: tc.name ?? "",
    };
  }

  if (t === "tool_call_response") {
    const tr = part as { id?: string; response?: unknown };
    const respParsed = maybeParseJson(tr.response);
    const respNode = (Array.isArray(respParsed) || isPlainObject(respParsed))
      ? jsonNode(respParsed, `${idPath}/response`, "response")
      : null;
    const primitive: { key: string; value: string }[] = [
      { key: "id", value: stringifyPrim(shortenId(tr.id ?? "")) },
    ];
    if (!respNode && tr.response !== undefined) {
      primitive.push({ key: "response", value: stringifyPrim(respParsed) });
    }
    return {
      id: idPath, type: "tool_call_response_part", label: "tool_call_response", bytes,
      children: respNode ? [respNode] : [], primitive, meta: shortenId(tr.id ?? ""),
    };
  }

  const node = jsonNode(part as unknown, idPath, t);
  if (node) return { ...node, type: "unknown_part", label: t, bytes };
  return { id: idPath, type: "unknown_part", label: t, bytes, children: [] };
}

function shortenId(id: string): string {
  if (!id) return "";
  if (id.length <= 14) return id;
  return id.slice(0, 6) + "…" + id.slice(-4);
}

function buildMessageNode(msg: Message, idPath: string): Node {
  const ibType = roleToIb(msg.role);
  const children = msg.parts.map((p, i) => buildPartNode(p, `${idPath}/parts/${i}`));
  const meta = msg.finish_reason ? `finish: ${msg.finish_reason}` : "";
  return {
    id: idPath, type: ibType, label: msg.role, bytes: safeBytes(msg),
    children, meta: meta || `${msg.parts.length} part${msg.parts.length === 1 ? "" : "s"}`,
  };
}

function buildToolDefNode(def: unknown, idPath: string): Node {
  if (isPlainObject(def)) {
    const name = typeof def.name === "string" ? def.name : "(unnamed)";
    const t = typeof def.type === "string" ? def.type : "tool";
    const desc = typeof def.description === "string" ? def.description : "";
    const paramsNode = isPlainObject(def.parameters) || Array.isArray(def.parameters)
      ? jsonNode(def.parameters, `${idPath}/parameters`, "parameters")
      : null;
    const primitive: { key: string; value: string }[] = [
      { key: "type", value: stringifyPrim(t) },
      { key: "name", value: stringifyPrim(name) },
    ];
    if (desc) primitive.push({ key: "description", value: stringifyPrim(desc) });
    return {
      id: idPath, type: "tool_def", label: name, bytes: safeBytes(def),
      children: paramsNode ? [paramsNode] : [], primitive, meta: t,
    };
  }
  const fb = jsonNode(def, idPath, "tool_def");
  if (fb) return { ...fb, type: "tool_def", label: "tool_def" };
  return { id: idPath, type: "tool_def", label: "tool_def", bytes: safeBytes(def), children: [] };
}

interface BuildArgs {
  systemParts: Part[];
  toolDefs: unknown[];
  inputMessages: Message[];
  outputMessages: Message[];
}

function buildTree({ systemParts, toolDefs, inputMessages, outputMessages }: BuildArgs): Node {
  const sysChildren: Node[] = systemParts.map((p, i) => {
    const t = typeof p.type === "string" ? p.type : "unknown";
    if (t === "text" || t === "reasoning") {
      const content = (p as { content?: string }).content ?? "";
      return {
        id: `root/system/parts/${i}`, type: "system_part", label: t,
        bytes: safeBytes(p), children: [],
        primitive: [{ key: "content", value: stringifyPrim(content) }],
        meta: `${content.length} ch`,
      };
    }
    const fb = jsonNode(p as unknown, `root/system/parts/${i}`, t);
    if (fb) return { ...fb, type: "system_part", label: t };
    return { id: `root/system/parts/${i}`, type: "system_part", label: t, bytes: safeBytes(p), children: [] };
  });
  const sysNode: Node = {
    id: "root/system", type: "system", label: "system instructions",
    bytes: safeBytes(systemParts), children: sysChildren,
    meta: `${systemParts.length} part${systemParts.length === 1 ? "" : "s"}`,
  };

  const tdChildren = toolDefs.map((d, i) => buildToolDefNode(d, `root/tool_defs/${i}`));
  const tdNode: Node = {
    id: "root/tool_defs", type: "tool_def_root", label: "tool definitions",
    bytes: safeBytes(toolDefs), children: tdChildren,
    meta: `${toolDefs.length} tool${toolDefs.length === 1 ? "" : "s"}`,
  };

  const inChildren = inputMessages.map((m, i) => buildMessageNode(m, `root/input_messages/${i}`));
  const inNode: Node = {
    id: "root/input_messages", type: "input_messages_root", label: "input messages",
    bytes: safeBytes(inputMessages), children: inChildren,
    meta: `${inputMessages.length} message${inputMessages.length === 1 ? "" : "s"}`,
  };

  const outChildren = outputMessages.map((m, i) => buildMessageNode(m, `root/output_messages/${i}`));
  const outNode: Node = {
    id: "root/output_messages", type: "output_messages_root", label: "output messages",
    bytes: safeBytes(outputMessages), children: outChildren,
    meta: `${outputMessages.length} message${outputMessages.length === 1 ? "" : "s"}`,
  };

  const rootChildren = [sysNode, tdNode, inNode, outNode];
  const rootBytes = rootChildren.reduce((a, c) => a + c.bytes, 0);
  return { id: "root", type: "root", label: "context input", bytes: rootBytes, children: rootChildren };
}

function walkVisible(node: Node, expanded: Set<string>, out: Node[]): void {
  if (node.children.length === 0 || !expanded.has(node.id)) { out.push(node); return; }
  for (const c of node.children) walkVisible(c, expanded, out);
}

// ---- Component ----

export function InputBreakdownScenario({ column }: { column: Column }) {
  const trace_id = column.config.selected_trace_id;
  const span_id = column.config.selected_span_id;

  const q = useQuery({
    queryKey: ["span", trace_id, span_id],
    queryFn: () => api.getSpan(trace_id!, span_id!),
    enabled: !!trace_id && !!span_id,
  });

  const detail = q.data;
  const isChat = detail?.span.kind_class === "chat";
  const a = useMemo(
    () => (isChat ? detail.span.attributes ?? {} : {}),
    [isChat, detail]
  );

  const tree = useMemo<Node | null>(() => {
    if (!isChat) return null;
    return buildTree({
      systemParts: parseSystemInstructions(a),
      toolDefs: parseToolDefinitions(a),
      inputMessages: parseInputMessages(a),
      outputMessages: parseOutputMessages(a),
    });
  }, [isChat, a]);

  const [expanded, setExpanded] = useState<Set<string>>(() => new Set<string>(["root"]));
  const [hoveredNodeId, setHoveredNodeId] = useState<string | null>(null);

  const toggle = (id: string) => {
    setExpanded((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  };

  const segments = useMemo<Node[]>(() => {
    if (!tree) return [];
    const out: Node[] = [];
    if (expanded.has(tree.id)) {
      for (const c of tree.children) walkVisible(c, expanded, out);
    } else out.push(tree);
    return out;
  }, [tree, expanded]);

  const totalBytes = tree ? Math.max(1, tree.bytes) : 1;
  const hasAny = isChat
    ? hasCapturedContent(a) || a["gen_ai.tool.definitions"] != null
    : false;

  return (
    <>
      <ColumnHeader column={column}>
        <span className="dim">span</span>
        <span className="mono">{span_id ? span_id.slice(0, 8) : "—"}</span>
        {detail && (
          <>
            <span className="dim">kind</span>
            <KindBadge k={detail.span.kind_class} />
          </>
        )}
      </ColumnHeader>

      <div
        className="col-body input-breakdown"
        style={{ display: "grid", gridTemplateRows: "20% 1fr", overflow: "hidden" }}
      >
        <div className="ib-summary">
          {!trace_id || !span_id ? (
            <div className="empty-state">Select a chat span in the Spans column.</div>
          ) : q.isLoading ? (
            <div className="empty-state">loading…</div>
          ) : !detail ? (
            <div className="empty-state">span not found</div>
          ) : !isChat ? (
            <div className="empty-state">selected span is not a chat span</div>
          ) : !hasAny ? (
            <div className="no-content">{NO_CONTENT_LINE}</div>
          ) : tree ? (
            <SummaryBar
              segments={segments}
              total={totalBytes}
              hoveredNodeId={hoveredNodeId}
              setHoveredNodeId={setHoveredNodeId}
              treeBytes={tree.bytes}
            />
          ) : null}
        </div>
        <div className="ib-tree-wrap" style={{ overflow: "auto", borderTop: "1px solid var(--border)" }}>
          {tree && hasAny && (
            <NodeView
              node={tree}
              depth={0}
              expanded={expanded}
              toggle={toggle}
              hoveredNodeId={hoveredNodeId}
              setHoveredNodeId={setHoveredNodeId}
            />
          )}
        </div>
      </div>
    </>
  );
}

function SummaryBar({
  segments, total, hoveredNodeId, setHoveredNodeId, treeBytes,
}: {
  segments: Node[]; total: number; hoveredNodeId: string | null;
  setHoveredNodeId: (s: string | null) => void; treeBytes: number;
}) {
  return (
    <div className="ib-summary-inner">
      <div className="ib-summary-label">
        <span>{fmtKB(treeBytes)} total</span>
        <span className="dim"> • {segments.length} block{segments.length === 1 ? "" : "s"}</span>
      </div>
      <div className="ib-bar">
        {segments.map((seg) => {
          const w = (Math.max(1, seg.bytes) / Math.max(1, total)) * 100;
          const hov = hoveredNodeId === seg.id;
          return (
            <div
              key={seg.id}
              className={`ib-seg ib-type-${seg.type}${hov ? " hovered" : ""}`}
              style={{ width: `${w}%` }}
              title={`${seg.label} — ${fmtKB(seg.bytes)}`}
              onMouseEnter={() => setHoveredNodeId(seg.id)}
              onMouseLeave={() => setHoveredNodeId(null)}
            />
          );
        })}
      </div>
    </div>
  );
}

function NodeView({
  node, depth, expanded, toggle, hoveredNodeId, setHoveredNodeId,
}: {
  node: Node; depth: number; expanded: Set<string>; toggle: (id: string) => void;
  hoveredNodeId: string | null; setHoveredNodeId: (s: string | null) => void;
}) {
  const isOpen = expanded.has(node.id);
  const hasChildren = node.children.length > 0;
  const hasPrim = !!node.primitive && node.primitive.length > 0;
  const collapsible = hasChildren || hasPrim;
  const hov = hoveredNodeId === node.id;
  const [expandedPrims, setExpandedPrims] = useState<Set<string>>(() => new Set());
  const togglePrim = (id: string) => {
    setExpandedPrims((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id); else next.add(id);
      return next;
    });
  };

  return (
    <div className={`ib-node ib-type-${node.type}`} style={{ marginLeft: depth === 0 ? 0 : 10 }}>
      <div
        className={`ib-header${hov ? " hovered" : ""}${collapsible ? " clickable" : ""}`}
        onClick={() => collapsible && toggle(node.id)}
        onMouseEnter={() => setHoveredNodeId(node.id)}
        onMouseLeave={() => setHoveredNodeId(null)}
      >
        <span className="ib-caret">{collapsible ? (isOpen ? "▾" : "▸") : "·"}</span>
        <span className="ib-chip">{node.type.replace(/_/g, " ")}</span>
        <span className="ib-label">{node.label}</span>
        {node.meta && <span className="ib-meta dim">{node.meta}</span>}
        <span className="ib-size">{fmtKB(node.bytes)}</span>
      </div>
      {isOpen && (
        <div className="ib-children">
          {hasPrim && (
            <div className="ib-prims">
              {node.primitive!.map((p, i) => {
                const pid = `${node.id}__p${i}`;
                const primOpen = expandedPrims.has(pid);
                const isLong = p.value.length > 200 || p.value.includes("\n");
                return (
                  <div className="ib-prim" key={pid}>
                    <span className="ib-prim-k">{p.key}</span>
                    <span
                      className={`ib-prim-v${isLong ? " ib-prim-v-clip" : ""}${isLong && primOpen ? " open" : ""}`}
                      onClick={isLong ? (e) => { e.stopPropagation(); togglePrim(pid); } : undefined}
                      title={isLong ? (primOpen ? "click to collapse" : "click to expand") : undefined}
                    >
                      {p.value}
                    </span>
                  </div>
                );
              })}
            </div>
          )}
          {node.children.map((c) => (
            <NodeView
              key={c.id}
              node={c}
              depth={depth + 1}
              expanded={expanded}
              toggle={toggle}
              hoveredNodeId={hoveredNodeId}
              setHoveredNodeId={setHoveredNodeId}
            />
          ))}
        </div>
      )}
    </div>
  );
}
