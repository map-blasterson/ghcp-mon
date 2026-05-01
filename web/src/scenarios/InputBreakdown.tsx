import { useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { diffWordsWithSpace, type Change } from "diff";
import { api } from "../api/client";
import type { Column } from "../state/workspace";
import type { SpanNode } from "../api/types";
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

// Tree node ids contain `/` characters which break querySelector
// attribute selectors unless escaped. Use the standards-track CSS.escape
// when present, with a small fallback for older runtimes.
function cssEscape(s: string): string {
  const fn = (globalThis as { CSS?: { escape?: (v: string) => string } }).CSS?.escape;
  if (typeof fn === "function") return fn(s);
  return s.replace(/[^a-zA-Z0-9_-]/g, (c) => `\\${c}`);
}

// ---- Tree model ----

type IBType =
  | "root"
  | "input_root"
  | "output_root"
  | "system"
  | "system_unchanged"
  | "system_diff"
  | "system_part"
  | "tool_def_root"
  | "tool_def_unchanged"
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

type Badge = "CHANGED" | "ADDED" | "REMOVED";

interface Node {
  id: string;
  type: IBType;
  label: string;
  bytes: number;
  children: Node[];
  primitive?: { key: string; value: string }[];
  meta?: string;
  badge?: Badge;
  diffSegments?: Change[];
}

type Mode = "DELTA" | "FULL";

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
  mode: Mode;
  prior: { systemParts: Part[]; toolDefs: unknown[] } | null;
}

// Deep equality for the captured content payloads. JSON.stringify is good
// enough here: messages/tool defs are JSON-shaped already and key order is
// stable across the parsing path (we don't reorder).
function deepEqualJson(a: unknown, b: unknown): boolean {
  return JSON.stringify(a) === JSON.stringify(b);
}

function toolName(d: unknown): string | null {
  if (isPlainObject(d) && typeof d.name === "string") return d.name;
  return null;
}

interface ToolDefDiff {
  added: unknown[];      // in current but not in prior, OR same name but content differs
  removed: unknown[];    // in prior but not in current, OR same name but content differs (paired w/ added)
  unchangedCount: number;
}

// Diff two tool-definition arrays by name.
//   - same name, same content   → unchanged
//   - same name, content differs → REMOVED (prior) + ADDED (current)
//   - name only in current      → ADDED
//   - name only in prior        → REMOVED
// Unnamed entries fall back to deep-equality multiset matching.
function diffToolDefs(current: unknown[], prior: unknown[]): ToolDefDiff {
  const byNameCur = new Map<string, unknown>();
  const byNamePrior = new Map<string, unknown>();
  const unnamedCur: unknown[] = [];
  const unnamedPrior: unknown[] = [];
  for (const d of current) {
    const n = toolName(d);
    if (n) byNameCur.set(n, d);
    else unnamedCur.push(d);
  }
  for (const d of prior) {
    const n = toolName(d);
    if (n) byNamePrior.set(n, d);
    else unnamedPrior.push(d);
  }
  const added: unknown[] = [];
  const removed: unknown[] = [];
  let unchangedCount = 0;
  for (const [n, cur] of byNameCur) {
    const pr = byNamePrior.get(n);
    if (pr === undefined) {
      added.push(cur);
    } else if (!deepEqualJson(cur, pr)) {
      removed.push(pr);
      added.push(cur);
    } else {
      unchangedCount++;
    }
  }
  for (const [n, pr] of byNamePrior) {
    if (!byNameCur.has(n)) removed.push(pr);
  }
  const usedPriorIdx = new Set<number>();
  for (const c of unnamedCur) {
    let matched = false;
    for (let i = 0; i < unnamedPrior.length; i++) {
      if (usedPriorIdx.has(i)) continue;
      if (deepEqualJson(unnamedPrior[i], c)) {
        usedPriorIdx.add(i);
        matched = true;
        unchangedCount++;
        break;
      }
    }
    if (!matched) added.push(c);
  }
  for (let i = 0; i < unnamedPrior.length; i++) {
    if (!usedPriorIdx.has(i)) removed.push(unnamedPrior[i]);
  }
  return { added, removed, unchangedCount };
}

function buildSystemPartChildren(systemParts: Part[], idBase: string): Node[] {
  return systemParts.map((p, i) => {
    const t = typeof p.type === "string" ? p.type : "unknown";
    if (t === "text" || t === "reasoning") {
      const content = (p as { content?: string }).content ?? "";
      return {
        id: `${idBase}/parts/${i}`, type: "system_part", label: t,
        bytes: safeBytes(p), children: [],
        primitive: [{ key: "content", value: stringifyPrim(content) }],
        meta: `${content.length} ch`,
      };
    }
    const fb = jsonNode(p as unknown, `${idBase}/parts/${i}`, t);
    if (fb) return { ...fb, type: "system_part", label: t };
    return { id: `${idBase}/parts/${i}`, type: "system_part", label: t, bytes: safeBytes(p), children: [] };
  });
}

// Concatenate all text/reasoning parts into a single string body. Multi-part
// system instructions are uncommon (Copilot CLI ships a single text part);
// when more than one part is present, separate them with a banner so the
// diff result is still legible.
function concatSystemBody(parts: Part[]): string {
  if (parts.length === 0) return "";
  if (parts.length === 1) {
    const p = parts[0] as { type?: string; content?: string };
    if (p.type === "text" || p.type === "reasoning") return p.content ?? "";
    return JSON.stringify(p);
  }
  const out: string[] = [];
  parts.forEach((raw, i) => {
    const p = raw as { type?: string; content?: string };
    out.push(`--- part ${i} (${p.type ?? "unknown"}) ---`);
    if (p.type === "text" || p.type === "reasoning") {
      out.push(p.content ?? "");
    } else {
      out.push(JSON.stringify(p));
    }
  });
  return out.join("\n");
}

function buildSystemNode(
  systemParts: Part[],
  mode: Mode,
  prior: BuildArgs["prior"],
): Node {
  const fullChildren = buildSystemPartChildren(systemParts, "root/input/system");
  const fullBytes = safeBytes(systemParts);
  const fullNode: Node = {
    id: "root/input/system", type: "system", label: "system instructions",
    bytes: fullBytes, children: fullChildren,
    meta: `${systemParts.length} part${systemParts.length === 1 ? "" : "s"}`,
  };
  if (mode === "FULL" || !prior) return fullNode;
  const same = deepEqualJson(systemParts, prior.systemParts);
  if (same) {
    return {
      id: "root/input/system", type: "system_unchanged", label: "system instructions",
      bytes: fullBytes, children: [],
      meta: `unchanged · ${systemParts.length} part${systemParts.length === 1 ? "" : "s"}`,
    };
  }
  // CHANGED: replace per-part full-content children with a single
  // `system_diff` child carrying word-level diff segments of the
  // concatenated body.
  const priorBody = concatSystemBody(prior.systemParts);
  const currentBody = concatSystemBody(systemParts);
  const segments = diffWordsWithSpace(priorBody, currentBody);
  const addedCh = segments.filter((s) => s.added).reduce((n, s) => n + s.value.length, 0);
  const remCh = segments.filter((s) => s.removed).reduce((n, s) => n + s.value.length, 0);
  const diffNode: Node = {
    id: "root/input/system/diff",
    type: "system_diff",
    label: "diff vs previous",
    bytes: currentBody.length,
    children: [],
    diffSegments: segments,
    meta: `+${addedCh} ch · -${remCh} ch`,
  };
  return {
    ...fullNode,
    children: [diffNode],
    badge: "CHANGED",
  };
}

function buildToolDefsNode(
  toolDefs: unknown[],
  mode: Mode,
  prior: BuildArgs["prior"],
): Node {
  const fullBytes = safeBytes(toolDefs);
  const fullChildren = toolDefs.map((d, i) =>
    buildToolDefNode(d, `root/input/tool_defs/${i}`),
  );
  const fullNode: Node = {
    id: "root/input/tool_defs", type: "tool_def_root", label: "tool definitions",
    bytes: fullBytes, children: fullChildren,
    meta: `${toolDefs.length} tool${toolDefs.length === 1 ? "" : "s"}`,
  };
  if (mode === "FULL" || !prior) return fullNode;
  const diff = diffToolDefs(toolDefs, prior.toolDefs);
  if (diff.added.length === 0 && diff.removed.length === 0) {
    return {
      id: "root/input/tool_defs", type: "tool_def_unchanged", label: "tool definitions",
      bytes: fullBytes, children: [],
      meta: `unchanged · ${toolDefs.length} tool${toolDefs.length === 1 ? "" : "s"}`,
    };
  }
  const children: Node[] = [];
  diff.removed.forEach((d, i) => {
    const node = buildToolDefNode(d, `root/input/tool_defs/rem/${i}`);
    children.push({ ...node, badge: "REMOVED" });
  });
  diff.added.forEach((d, i) => {
    const node = buildToolDefNode(d, `root/input/tool_defs/add/${i}`);
    children.push({ ...node, badge: "ADDED" });
  });
  const metaParts: string[] = [];
  if (diff.added.length) metaParts.push(`${diff.added.length} added`);
  if (diff.removed.length) metaParts.push(`${diff.removed.length} removed`);
  if (diff.unchangedCount) metaParts.push(`${diff.unchangedCount} unchanged`);
  return {
    id: "root/input/tool_defs", type: "tool_def_root", label: "tool definitions",
    bytes: fullBytes, children,
    meta: metaParts.join(" · "),
  };
}

function buildTree({
  systemParts, toolDefs, inputMessages, outputMessages, mode, prior,
}: BuildArgs): Node {
  const sysNode = buildSystemNode(systemParts, mode, prior);
  const tdNode = buildToolDefsNode(toolDefs, mode, prior);

  const inChildren = inputMessages.map((m, i) =>
    buildMessageNode(m, `root/input/input_messages/${i}`),
  );
  const inMsgsNode: Node = {
    id: "root/input/input_messages", type: "input_messages_root", label: "input messages",
    bytes: safeBytes(inputMessages), children: inChildren,
    meta: `${inputMessages.length} message${inputMessages.length === 1 ? "" : "s"} · per-turn delta`,
  };

  const inputChildren = [sysNode, tdNode, inMsgsNode];
  const inputBytes = inputChildren.reduce((a, c) => a + c.bytes, 0);
  const inputRoot: Node = {
    id: "root/input", type: "input_root", label: "context input",
    bytes: inputBytes, children: inputChildren,
  };

  const outChildren = outputMessages.map((m, i) =>
    buildMessageNode(m, `root/output/${i}`),
  );
  const outputBytes = safeBytes(outputMessages);
  const outputRoot: Node = {
    id: "root/output", type: "output_root", label: "context output",
    bytes: outputBytes, children: outChildren,
    meta: `${outputMessages.length} message${outputMessages.length === 1 ? "" : "s"}`,
  };

  return {
    id: "root", type: "root", label: "chat detail",
    bytes: inputBytes + outputBytes,
    children: [inputRoot, outputRoot],
  };
}

function walkVisible(node: Node, expanded: Set<string>, out: Node[]): void {
  if (node.children.length === 0 || !expanded.has(node.id)) { out.push(node); return; }
  for (const c of node.children) walkVisible(c, expanded, out);
}

// Walk a span tree and collect chat-class spans, sorted by end time
// ascending (with start fallback for in-progress spans). Used to find
// the chat span immediately preceding the selected one in the same
// session.
function flatChatSpansSorted(tree: SpanNode[]): SpanNode[] {
  const out: SpanNode[] = [];
  const walk = (nodes: SpanNode[]) => {
    for (const n of nodes) {
      if (n.kind_class === "chat") out.push(n);
      walk(n.children ?? []);
    }
  };
  walk(tree);
  out.sort((a, b) => {
    const ka = a.end_unix_ns ?? a.start_unix_ns ?? 0;
    const kb = b.end_unix_ns ?? b.start_unix_ns ?? 0;
    if (ka !== kb) return ka - kb;
    return (a.span_pk ?? 0) - (b.span_pk ?? 0);
  });
  return out;
}

// ---- Component ----

export function InputBreakdownScenario({ column }: { column: Column }) {
  const trace_id = column.config.selected_trace_id;
  const span_id = column.config.selected_span_id;
  const [mode, setMode] = useState<Mode>("DELTA");

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

  // Resolve the conversation_id from the current chat span. Without it
  // we cannot locate prior chat spans, and DELTA mode degrades to FULL.
  const cid = useMemo(() => {
    const v = (a as Record<string, unknown>)["gen_ai.conversation.id"];
    return typeof v === "string" ? v : null;
  }, [a]);

  const sessionTreeQ = useQuery({
    queryKey: ["session-span-tree", cid],
    queryFn: () => api.getSessionSpanTree(cid!),
    enabled: !!cid && mode === "DELTA",
  });

  // Prior chat span: the chat-kind span immediately preceding the
  // currently selected one in end-time order across the whole session.
  const priorRef = useMemo<SpanNode | null>(() => {
    if (mode !== "DELTA" || !sessionTreeQ.data || !span_id) return null;
    const all = flatChatSpansSorted(sessionTreeQ.data.tree);
    const idx = all.findIndex((s) => s.span_id === span_id);
    if (idx <= 0) return null;
    return all[idx - 1];
  }, [mode, sessionTreeQ.data, span_id]);

  const priorSpanQ = useQuery({
    queryKey: ["span", priorRef?.trace_id, priorRef?.span_id],
    queryFn: () => api.getSpan(priorRef!.trace_id, priorRef!.span_id),
    enabled: !!priorRef,
  });

  const prior = useMemo<BuildArgs["prior"]>(() => {
    if (mode !== "DELTA") return null;
    const pa = priorSpanQ.data?.span.attributes ?? null;
    if (!pa) return null;
    // If the prior span has no captured content, fall back to FULL for
    // system/tool_defs (treat as if there were no prior).
    const sys = parseSystemInstructions(pa);
    const tools = parseToolDefinitions(pa);
    if (sys.length === 0 && tools.length === 0) return null;
    return { systemParts: sys, toolDefs: tools };
  }, [mode, priorSpanQ.data]);

  const tree = useMemo<Node | null>(() => {
    if (!isChat) return null;
    return buildTree({
      systemParts: parseSystemInstructions(a),
      toolDefs: parseToolDefinitions(a),
      inputMessages: parseInputMessages(a),
      outputMessages: parseOutputMessages(a),
      mode,
      prior,
    });
  }, [isChat, a, mode, prior]);

  const [expanded, setExpanded] = useState<Set<string>>(
    () => new Set<string>(["root", "root/input"]),
  );
  const [hoveredNodeId, setHoveredNodeId] = useState<string | null>(null);

  // Tool-call hint: when Spans.tsx auto-routed us here in response to
  // an execute_tool selection, locate the matching message_tool node
  // (the tool message containing a tool_call_response with this id)
  // so we can auto-expand its ancestors and visually mark it.
  const selectedToolCallId = column.config.selected_tool_call_id;
  const targetMessageNodeId = useMemo<string | null>(() => {
    if (!selectedToolCallId || !isChat) return null;
    const msgs = parseInputMessages(a);
    for (let i = 0; i < msgs.length; i++) {
      const m = msgs[i];
      if (m.role !== "tool") continue;
      for (const p of m.parts) {
        const pt = p as { type?: unknown; id?: unknown };
        if (pt.type === "tool_call_response" && pt.id === selectedToolCallId) {
          return `root/input/input_messages/${i}`;
        }
      }
    }
    return null;
  }, [selectedToolCallId, isChat, a]);

  // Auto-expand the ancestor chain leading to the targeted message_tool
  // node so the arrow lands on a visible row. Only adds; never collapses
  // anything the user already opened.
  useEffect(() => {
    if (!targetMessageNodeId) return;
    setExpanded((prev) => {
      const next = new Set(prev);
      next.add("root");
      next.add("root/input");
      next.add("root/input/input_messages");
      next.add(targetMessageNodeId);
      return next;
    });
  }, [targetMessageNodeId]);

  const treeWrapRef = useRef<HTMLDivElement | null>(null);
  const [arrowTop, setArrowTop] = useState<number | null>(null);

  // Compute the y-position of the arrow inside the scrollable tree
  // wrap by reading the targeted header's offset relative to the wrap.
  // Recompute whenever the target id changes, the tree rerenders, or
  // expand state changes (which can move the row up/down).
  useLayoutEffect(() => {
    if (!targetMessageNodeId) {
      setArrowTop(null);
      return;
    }
    const wrap = treeWrapRef.current;
    if (!wrap) return;
    const el = wrap.querySelector<HTMLElement>(
      `[data-ib-id="${cssEscape(targetMessageNodeId)}"]`,
    );
    if (!el) {
      setArrowTop(null);
      return;
    }
    const wrapRect = wrap.getBoundingClientRect();
    const elRect = el.getBoundingClientRect();
    // Place the arrow vertically centered on the row, accounting for
    // the wrap's own scroll offset so the arrow follows scrolling.
    const top = elRect.top - wrapRect.top + wrap.scrollTop + elRect.height / 2;
    setArrowTop(top);
  }, [targetMessageNodeId, tree, expanded]);

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
        {isChat && (
          <span className="ib-mode-toggle" role="group" aria-label="view mode">
            <button
              type="button"
              className={`ib-mode-btn${mode === "DELTA" ? " active" : ""}`}
              onClick={() => setMode("DELTA")}
            >
              DELTA
            </button>
            <button
              type="button"
              className={`ib-mode-btn${mode === "FULL" ? " active" : ""}`}
              onClick={() => setMode("FULL")}
            >
              FULL
            </button>
          </span>
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
        <div
          ref={treeWrapRef}
          className="ib-tree-wrap"
          style={{ overflow: "auto", borderTop: "1px solid var(--border)", position: "relative" }}
        >
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
          {arrowTop != null && (
            <div
              className="ib-target-arrow"
              style={{ top: arrowTop }}
              aria-hidden
            >
              ▶
            </div>
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
  const hasDiff = !!node.diffSegments && node.diffSegments.length > 0;
  const collapsible = hasChildren || hasPrim || hasDiff;
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
        data-ib-id={node.id}
        onClick={() => collapsible && toggle(node.id)}
        onMouseEnter={() => setHoveredNodeId(node.id)}
        onMouseLeave={() => setHoveredNodeId(null)}
      >
        <span className="ib-caret">{collapsible ? (isOpen ? "▾" : "▸") : "·"}</span>
        <span className="ib-chip">{node.type.replace(/_/g, " ")}</span>
        <span className="ib-label">{node.label}</span>
        {node.badge && (
          <span className={`tag ib-badge-${node.badge.toLowerCase()}`}>{node.badge}</span>
        )}
        {node.meta && <span className="ib-meta dim">{node.meta}</span>}
        <span className="ib-size">{fmtKB(node.bytes)}</span>
      </div>
      {isOpen && (
        <div className="ib-children">
          {hasDiff && (
            <div className="ib-diff" onClick={(e) => e.stopPropagation()}>
              {node.diffSegments!.map((seg, i) => {
                const cls = seg.added
                  ? "ib-diff-add"
                  : seg.removed
                  ? "ib-diff-rem"
                  : "ib-diff-eq";
                return (
                  <span key={i} className={cls}>
                    {seg.value}
                  </span>
                );
              })}
            </div>
          )}
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
