// Typed extractors for OTel GenAI latest-experimental content attributes.
//
// Spec references (./reference/otel-semconv-genai.md and linked sub-pages):
//   - gen-ai-spans       https://opentelemetry.io/docs/specs/semconv/gen-ai/gen-ai-spans/
//     Inference span notes [16] (gen_ai.input.messages),
//                          [17] (gen_ai.output.messages),
//                          [18] (gen_ai.system_instructions).
//     Execute-tool span notes [4] (gen_ai.tool.call.arguments),
//                             [5] (gen_ai.tool.call.result).
//   - gen-ai-agent-spans https://opentelemetry.io/docs/specs/semconv/gen-ai/gen-ai-agent-spans/
//     Note [14] — invoke_agent spans carry the same input/output/system
//     content attributes as inference spans.
//   - JSON schemas for the three message-set attributes:
//       gen-ai-input-messages.json, gen-ai-output-messages.json,
//       gen-ai-system-instructions.json
//
// All four content attributes ship as JSON-stringified arrays. Per the
// schemas: messages are `{role, parts:[…], finish_reason?}`. Parts have a
// `type` discriminator; the types this dashboard renders specifically are
// "text", "reasoning", "tool_call", "tool_call_response". Unknown part
// types are passed through as-is.

export const NO_CONTENT_LINE =
  "no content captured — set OTEL_INSTRUMENTATION_GENAI_CAPTURE_MESSAGE_CONTENT=true and OTEL_SEMCONV_STABILITY_OPT_IN=gen_ai_latest_experimental";

export type Role = "user" | "assistant" | "system" | "tool";

export interface TextPart {
  type: "text";
  content: string;
}
export interface ReasoningPart {
  type: "reasoning";
  content: string;
}
export interface ToolCallPart {
  type: "tool_call";
  id: string;
  name: string;
  arguments: unknown;
}
export interface ToolCallResponsePart {
  type: "tool_call_response";
  id: string;
  response: unknown;
}
export interface UnknownPart {
  type: string;
  [k: string]: unknown;
}
export type Part =
  | TextPart
  | ReasoningPart
  | ToolCallPart
  | ToolCallResponsePart
  | UnknownPart;

export interface Message {
  role: Role | string;
  parts: Part[];
  finish_reason?: string;
}

// ---- entry points ----

/** Extract the (already-parsed) attribute bag for a span. Some call sites
 *  carry the row as `{attributes: {...}}` (the live API shape) and some as
 *  `{attributes_json: "..."}` (the raw DB shape). Accept both. */
export function attrs(
  span: { attributes?: Record<string, unknown> | null; attributes_json?: string | null } | null | undefined
): Record<string, unknown> {
  if (!span) return {};
  if (span.attributes && typeof span.attributes === "object") return span.attributes;
  if (typeof span.attributes_json === "string" && span.attributes_json.length > 0) {
    return parseJsonObject(span.attributes_json, "attributes_json") ?? {};
  }
  return {};
}

export function parseInputMessages(a: Record<string, unknown>): Message[] {
  return parseMessageArray(a["gen_ai.input.messages"], "gen_ai.input.messages");
}

export function parseOutputMessages(a: Record<string, unknown>): Message[] {
  return parseMessageArray(a["gen_ai.output.messages"], "gen_ai.output.messages");
}

export function parseSystemInstructions(a: Record<string, unknown>): Part[] {
  const raw = a["gen_ai.system_instructions"];
  const arr = parseMaybeJsonArray(raw, "gen_ai.system_instructions");
  if (!arr) return [];
  return arr.map(normalizePart).filter(Boolean) as Part[];
}

export function parseToolCallArguments(a: Record<string, unknown>): unknown {
  const raw = a["gen_ai.tool.call.arguments"];
  if (raw == null) return null;
  if (typeof raw === "string") {
    try {
      return JSON.parse(raw);
    } catch {
      // Per execute-tool note [4] this SHOULD be a JSON object string, but
      // tolerate a non-JSON string by returning it verbatim.
      return raw;
    }
  }
  return raw;
}

export function parseToolCallResult(a: Record<string, unknown>): string | unknown {
  const raw = a["gen_ai.tool.call.result"];
  if (raw == null) return null;
  if (typeof raw === "string") {
    // Per note [5] this is opaque — the tool decides. For Copilot's bash
    // family it's a literal stdout/stderr blob with a trailing
    // `<exited with exit code N>`. Try to JSON-parse for tools that emit
    // structured results, but if parsing yields a string OR fails, return
    // the raw string so callers can render it as a single block.
    try {
      const v = JSON.parse(raw);
      return typeof v === "string" ? raw : v;
    } catch {
      return raw;
    }
  }
  return raw;
}

/** True iff this span has any of the four GenAI content attributes set. */
export function hasCapturedContent(a: Record<string, unknown>): boolean {
  return (
    a["gen_ai.input.messages"] != null ||
    a["gen_ai.output.messages"] != null ||
    a["gen_ai.system_instructions"] != null
  );
}

// ---- internals ----

function parseMessageArray(raw: unknown, label: string): Message[] {
  const arr = parseMaybeJsonArray(raw, label);
  if (!arr) return [];
  const out: Message[] = [];
  for (const m of arr) {
    if (!m || typeof m !== "object") continue;
    const obj = m as Record<string, unknown>;
    const role = typeof obj.role === "string" ? obj.role : "unknown";
    const partsRaw = obj.parts;
    const parts: Part[] = [];
    if (Array.isArray(partsRaw)) {
      for (const p of partsRaw) {
        const np = normalizePart(p);
        if (np) parts.push(np);
      }
    }
    const msg: Message = { role, parts };
    if (typeof obj.finish_reason === "string") msg.finish_reason = obj.finish_reason;
    out.push(msg);
  }
  return out;
}

function normalizePart(p: unknown): Part | null {
  if (!p || typeof p !== "object") return null;
  const obj = { ...(p as Record<string, unknown>) };
  const type = typeof obj.type === "string" ? obj.type : "unknown";

  if (type === "tool_call") {
    let args = obj.arguments;
    if (typeof args === "string") {
      try {
        args = JSON.parse(args);
      } catch {
        // leave as string
      }
    }
    return {
      type: "tool_call",
      id: typeof obj.id === "string" ? obj.id : "",
      name: typeof obj.name === "string" ? obj.name : "",
      arguments: args,
    };
  }

  if (type === "tool_call_response") {
    let resp = obj.response;
    if (typeof resp === "string") {
      try {
        const v = JSON.parse(resp);
        // keep raw string if it parses to a primitive string
        resp = typeof v === "string" ? resp : v;
      } catch {
        // not JSON, leave verbatim
      }
    }
    return {
      type: "tool_call_response",
      id: typeof obj.id === "string" ? obj.id : "",
      response: resp,
    };
  }

  if (type === "text" || type === "reasoning") {
    const content = typeof obj.content === "string" ? obj.content : "";
    return { type, content } as TextPart | ReasoningPart;
  }

  return obj as UnknownPart;
}

let warned = new Set<string>();
function warnOnce(label: string, err: unknown): void {
  if (warned.has(label)) return;
  warned.add(label);
  // eslint-disable-next-line no-console
  console.warn(`[content] failed to parse ${label}:`, err);
}

function parseMaybeJsonArray(raw: unknown, label: string): unknown[] | null {
  if (raw == null) return null;
  if (Array.isArray(raw)) return raw;
  if (typeof raw === "string") {
    try {
      const v = JSON.parse(raw);
      return Array.isArray(v) ? v : null;
    } catch (e) {
      warnOnce(label, e);
      return null;
    }
  }
  return null;
}

function parseJsonObject(raw: string, label: string): Record<string, unknown> | null {
  try {
    const v = JSON.parse(raw);
    return v && typeof v === "object" && !Array.isArray(v) ? (v as Record<string, unknown>) : null;
  } catch (e) {
    warnOnce(label, e);
    return null;
  }
}

// ---- shell-tool family detection ----

// ---- formatting helpers (unchanged) ----

export function fmtNs(ns: number | null | undefined): string {
  if (ns == null) return "—";
  const ms = ns / 1_000_000;
  if (ms >= 1000) return `${(ms / 1000).toFixed(2)}s`;
  if (ms >= 1) return `${ms.toFixed(1)}ms`;
  return `${ns}ns`;
}

export function fmtClock(ns: number | null | undefined): string {
  if (ns == null) return "—";
  const d = new Date(ns / 1_000_000);
  return d.toLocaleTimeString();
}

export function fmtRelative(ns: number | null | undefined): string {
  if (ns == null) return "—";
  const ms = Date.now() - ns / 1_000_000;
  if (ms < 0) return "now";
  if (ms < 1000) return "now";
  if (ms < 60_000) return `${Math.floor(ms / 1000)}s ago`;
  if (ms < 3_600_000) return `${Math.floor(ms / 60_000)}m ago`;
  if (ms < 86_400_000) return `${Math.floor(ms / 3_600_000)}h ago`;
  return `${Math.floor(ms / 86_400_000)}d ago`;
}

export function prettyJson(v: unknown): string {
  try {
    return JSON.stringify(v, null, 2);
  } catch {
    return String(v);
  }
}
