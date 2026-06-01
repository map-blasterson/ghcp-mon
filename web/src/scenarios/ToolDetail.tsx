import { useMemo, type ReactNode } from "react";
import { useQuery } from "@tanstack/react-query";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import Prism from "prismjs";
import { diffLines, diffWordsWithSpace } from "diff";
import { api } from "../api/client";
import type { Column } from "../state/workspace";
import { ColumnHeader } from "../components/ColumnHeader";
import { JsonView } from "../components/JsonView";
import { CodeBlock, langFromPath } from "../components/CodeBlock";
import { TextBlock } from "../components/TextBlock";
import {
  fmtNs,
  fmtClock,
  NO_CONTENT_LINE,
  parseToolCallArguments,
  parseToolCallResult,
  prettyJson,
} from "../components/content";
import type { SpanDetail } from "../api/types";

export function ToolDetailScenario({ column }: { column: Column }) {
  const trace_id = column.config.selected_trace_id;
  const span_id = column.config.selected_span_id;
  const searchQuery = (column.config.search_query as string | undefined) || undefined;

  const q = useQuery({
    queryKey: ["span", trace_id, span_id],
    queryFn: () => api.getSpan(trace_id!, span_id!),
    enabled: !!trace_id && !!span_id,
  });

  return (
    <>
      <ColumnHeader column={column}>
        <span className="dim">span</span>
        <span className="mono">{span_id ? span_id.slice(0, 8) : "—"}</span>
      </ColumnHeader>
      <div className="col-body">
        {!trace_id || !span_id ? (
          <div className="empty-state">Select a tool span in the Spans column.</div>
        ) : q.isLoading ? (
          <div className="empty-state">loading…</div>
        ) : !q.data ? (
          <div className="empty-state">span not found</div>
        ) : q.data.projection.tool_call ? (
          <ToolDetailBody detail={q.data} externalQuery={searchQuery} />
        ) : q.data.projection.external_tool_call ? (
          <ExternalToolDetailBody detail={q.data} externalQuery={searchQuery} />
        ) : (
          <div className="empty-state">selected span is not a tool call</div>
        )}
      </div>
    </>
  );
}

// Priority-ordered list of argument keys whose value, when present on a
// function-typed tool call, is hoisted into a prominent "hero" panel so
// the most-actionable parameter is visible at a glance even with the
// collapsible metadata panel closed. Only the first matching key is
// surfaced.
const HERO_KEYS = ["command", "query", "description"];

function pickHero(
  argsObj: Record<string, unknown> | null,
): { key: string; value: string } | null {
  if (!argsObj) return null;
  for (const k of HERO_KEYS) {
    if (!(k in argsObj)) continue;
    const v = argsObj[k];
    if (v == null) continue;
    const sv = typeof v === "string" ? v : prettyJson(v);
    if (sv.length === 0) continue;
    return { key: k, value: sv };
  }
  return null;
}

function ToolDetailBody({ detail, externalQuery }: { detail: SpanDetail; externalQuery?: string }) {
  const tc = detail.projection.tool_call!;
  const span = detail.span;
  const dur =
    span.duration_ns ??
    (span.start_unix_ns != null && span.end_unix_ns != null
      ? span.end_unix_ns - span.start_unix_ns
      : null);
  const a = span.attributes ?? {};
  const args = parseToolCallArguments(a);
  const argsObj =
    args && typeof args === "object" && !Array.isArray(args)
      ? (args as Record<string, unknown>)
      : null;
  const hero = tc.tool_type === "function" ? pickHero(argsObj) : null;
  return (
    <>
      <details className="section">
        <summary><h4>{tc.tool_name ?? "(unknown tool)"}</h4></summary>
        <div className="kv">
          <span className="k">call_id</span>
          <span className="v mono">{tc.call_id ?? "—"}</span>
          <span className="k">tool_type</span>
          <span className="v">{tc.tool_type ?? "—"}</span>
          <span className="k">duration</span>
          <span className="v">{fmtNs(dur)}</span>
          <span className="k">status</span>
          <span className="v">{tc.status_code ?? "—"}</span>
          <span className="k">start</span>
          <span className="v">{fmtClock(span.start_unix_ns)}</span>
          <span className="k">conv</span>
          <span className="v mono">{tc.conversation_id?.slice(0, 8) ?? "—"}</span>
        </div>
      </details>
      {hero && (
        <div className="section tool-hero">
          <pre className="tool-hero-value">{hero.value}</pre>
        </div>
      )}
      <div className="section">
        <h4>args / result</h4>
        {tc.tool_name === "edit" || tc.tool_name === "write" ? (
          <EditArgs attributes={a} externalQuery={externalQuery} />
        ) : tc.tool_name === "view" || tc.tool_name === "read" ? (
          <ViewArgs attributes={a} externalQuery={externalQuery} />
        ) : tc.tool_name === "read_agent" ? (
          <ReadAgentArgs attributes={a} externalQuery={externalQuery} />
        ) : tc.tool_name === "task" ? (
          <TaskArgs attributes={a} externalQuery={externalQuery} />
        ) : (
          <GenericArgs attributes={a} externalQuery={externalQuery} />
        )}
      </div>
      <div className="section">
        <h4>raw span attributes</h4>
        <JsonView value={span.attributes} collapsed />
      </div>
    </>
  );
}

// Renders a span backed only by an `external_tool_call` projection
// (MCP / external-origin tool spans, which have no `tool_call` row).
// Mirrors the kv layout of ToolDetailBody but uses the fields actually
// present on ExternalToolCallProjection. Falls back to GenericArgs for
// args/result since the tool_name comes from an external source and we
// don't have specialized renderers for it.
function ExternalToolDetailBody({ detail, externalQuery }: { detail: SpanDetail; externalQuery?: string }) {
  const ext = detail.projection.external_tool_call!;
  const span = detail.span;
  const dur =
    span.duration_ns ??
    (span.start_unix_ns != null && span.end_unix_ns != null
      ? span.end_unix_ns - span.start_unix_ns
      : null);
  const a = span.attributes ?? {};
  return (
    <>
      <details className="section">
        <summary><h4>{ext.tool_name ?? "(unknown tool)"}</h4></summary>
        <div className="kv">
          <span className="k">call_id</span>
          <span className="v mono">{ext.call_id ?? "—"}</span>
          <span className="k">tool_type</span>
          <span className="v">external</span>
          <span className="k">duration</span>
          <span className="v">{fmtNs(dur)}</span>
          <span className="k">start</span>
          <span className="v">{fmtClock(span.start_unix_ns)}</span>
          <span className="k">conv</span>
          <span className="v mono">{ext.conversation_id?.slice(0, 8) ?? "—"}</span>
          <span className="k">paired_tool_call_pk</span>
          <span className="v mono">{ext.paired_tool_call_pk ?? "—"}</span>
          <span className="k">agent_run_pk</span>
          <span className="v mono">{ext.agent_run_pk ?? "—"}</span>
        </div>
      </details>
      <div className="section">
        <h4>args / result</h4>
        <GenericArgs attributes={a} externalQuery={externalQuery} />
      </div>
      <div className="section">
        <h4>raw span attributes</h4>
        <JsonView value={span.attributes} collapsed />
      </div>
    </>
  );
}

function GenericArgs({ attributes, externalQuery }: { attributes: Record<string, unknown>; externalQuery?: string }) {
  const args = parseToolCallArguments(attributes);
  const result = parseToolCallResult(attributes);
  if (args == null && result == null) return <div className="no-content">{NO_CONTENT_LINE}</div>;

  const argsObj =
    args && typeof args === "object" && !Array.isArray(args)
      ? (args as Record<string, unknown>)
      : null;

  // Split args into "code-ish" string fields (anything with a newline)
  // and the rest. Code-ish fields render as <pre>; the rest fall back
  // to JSON pretty-print so structured args (objects, arrays, ints,
  // bools) still look right.
  const codeFields: Array<[string, string]> = [];
  let restObj: Record<string, unknown> | null = null;
  if (argsObj) {
    const rest: Record<string, unknown> = {};
    for (const [k, v] of Object.entries(argsObj)) {
      if (typeof v === "string" && v.includes("\n")) codeFields.push([k, v]);
      else rest[k] = v;
    }
    if (Object.keys(rest).length > 0) restObj = rest;
  }

  return (
    <>
      {args != null && (
        <div className="shell">
          <div className="label">arguments</div>
          {argsObj ? (
            <>
              {codeFields.map(([k, v]) => (
                <div key={`code-${k}`}>
                  <div className="label" style={{ marginTop: 4 }}>{k}</div>
                  <TextBlock searchable text={v} preClassName="edit-diff" externalQuery={externalQuery} />
                </div>
              ))}
              {restObj && (
                <TextBlock searchable externalQuery={externalQuery}>
                  <pre className="json" style={{ marginTop: codeFields.length ? 4 : 0 }}>
                    {prettyJson(restObj)}
                  </pre>
                </TextBlock>
              )}
              {!restObj && codeFields.length === 0 && (
                <TextBlock searchable text={prettyJson(args)} preClassName="json" externalQuery={externalQuery} />
              )}
            </>
          ) : (
            <TextBlock searchable text={prettyJson(args)} preClassName="json" externalQuery={externalQuery} />
          )}
        </div>
      )}
      {result != null && (
        <div className="shell">
          <div className="label">result</div>
          {typeof result === "string" ? (
            <TextBlock searchable text={result} externalQuery={externalQuery} />
          ) : (
            <TextBlock searchable text={prettyJson(result)} preClassName="json" externalQuery={externalQuery} />
          )}
        </div>
      )}
    </>
  );
}

// Specialized view for file-mutating tools (`edit`, `write`). Renders
//   - path           plain (accepts `path` or `filePath`)
//   - old_str        red, with newlines preserved verbatim (edit only;
//                    accepts `old_str` or opencode's `oldString`)
//   - new_str/content green, with newlines preserved verbatim
//                    (accepts `new_str`, opencode's `newString`, or
//                    opencode write's `content`)
//   - any other args fall through as JSON
//   - result         same fallback as GenericArgs
//
// `edit` is a diff-shaped update (old + new both present). `write` is a
// new-file create (content only); the label switches to "content" in
// that case so the panel matches the source tool.
function EditArgs({ attributes, externalQuery }: { attributes: Record<string, unknown>; externalQuery?: string }) {
  const args = parseToolCallArguments(attributes);
  const result = parseToolCallResult(attributes);
  const argsObj =
    args && typeof args === "object" && !Array.isArray(args)
      ? (args as Record<string, unknown>)
      : null;
  if (!argsObj && result == null) return <div className="no-content">{NO_CONTENT_LINE}</div>;
  const path = argsObj && typeof argsObj.path === "string"
    ? argsObj.path
    : argsObj && typeof argsObj.filePath === "string"
      ? argsObj.filePath
      : null;
  const oldStr = argsObj && typeof argsObj.old_str === "string"
    ? argsObj.old_str
    : argsObj && typeof argsObj.oldString === "string"
      ? argsObj.oldString
      : null;
  const newStr = argsObj && typeof argsObj.new_str === "string"
    ? argsObj.new_str
    : argsObj && typeof argsObj.newString === "string"
      ? argsObj.newString
      : argsObj && typeof argsObj.content === "string"
        ? argsObj.content
        : null;
  // When the body came from `content` (opencode `write`), label the
  // panel accordingly; otherwise keep `new_str` for parity with `edit`.
  const newStrLabel =
    argsObj && typeof argsObj.new_str !== "string"
      && typeof argsObj.newString !== "string"
      && typeof argsObj.content === "string"
      ? "content"
      : "new_str";
  const lang = langFromPath(path);
  const extraEntries = argsObj
    ? Object.entries(argsObj).filter(
        ([k]) => k !== "path" && k !== "filePath"
          && k !== "old_str" && k !== "oldString"
          && k !== "new_str" && k !== "newString"
          && k !== "content"
      )
    : [];
  const extraObj = extraEntries.length ? Object.fromEntries(extraEntries) : null;
  return (
    <>
      {argsObj && (
        <div className="shell">
          <div className="label">arguments</div>
          {path != null && (
            <div className="kv" style={{ padding: "2px 0" }}>
              <span className="k">path</span>
              <span className="v mono">{path}</span>
            </div>
          )}
          {oldStr != null && newStr != null ? (
            <>
              <div className="label" style={{ marginTop: 4 }}>diff</div>
              <InlineDiff
                oldStr={oldStr}
                newStr={newStr}
                language={lang}
                externalQuery={externalQuery}
              />
            </>
          ) : (
            <>
              {oldStr != null && (
                <>
                  <div className="label" style={{ marginTop: 4 }}>old_str</div>
                  <TextBlock searchable externalQuery={externalQuery}>
                    <CodeBlock
                      language={lang}
                      text={oldStr}
                      className="edit-diff edit-diff-old"
                    />
                  </TextBlock>
                </>
              )}
              {newStr != null && (
                <>
                  <div className="label" style={{ marginTop: 4 }}>{newStrLabel}</div>
                  <TextBlock searchable externalQuery={externalQuery}>
                    <CodeBlock
                      language={lang}
                      text={newStr}
                      className="edit-diff edit-diff-new"
                    />
                  </TextBlock>
                </>
              )}
            </>
          )}
          {extraObj && (
            <>
              <div className="label" style={{ marginTop: 4 }}>other</div>
              <TextBlock searchable text={prettyJson(extraObj)} preClassName="json" externalQuery={externalQuery} />
            </>
          )}
        </div>
      )}
      {result != null && (
        <div className="shell">
          <div className="label">result</div>
          {renderEditResult(result, externalQuery)}
        </div>
      )}
    </>
  );
}

// Pick the most informative renderer for an `edit` / `write` tool
// result. opencode's edit result is `{title, output, metadata: {diff,
// filediff, diagnostics}}` — the unified-diff body in `metadata.diff`
// is what the user wants to see, not the full envelope. opencode's
// write result is `{title, output: "Wrote file successfully.",
// metadata}` — `output` is the only useful string. Copilot's results
// are plain strings (or already-structured JSON without these
// envelopes) and pass through unchanged.
function renderEditResult(result: unknown, externalQuery?: string) {
  if (typeof result === "string") {
    return <TextBlock searchable text={result} externalQuery={externalQuery} />;
  }
  if (result && typeof result === "object" && !Array.isArray(result)) {
    const obj = result as Record<string, unknown>;
    const meta = obj.metadata;
    if (meta && typeof meta === "object" && !Array.isArray(meta)) {
      const diff = (meta as Record<string, unknown>).diff;
      if (typeof diff === "string" && diff.length > 0) {
        return (
          <TextBlock searchable externalQuery={externalQuery}>
            <UnifiedDiff text={diff} />
          </TextBlock>
        );
      }
    }
    const output = obj.output;
    if (typeof output === "string" && output.length > 0) {
      return <TextBlock searchable text={output} externalQuery={externalQuery} />;
    }
  }
  return <TextBlock searchable text={prettyJson(result)} preClassName="json" externalQuery={externalQuery} />;
}

// Render a unified-diff body with per-line coloring. Treats `+++`/
// `---` file headers and `@@` hunk headers distinctly from added/
// removed lines so the result looks like a familiar diff viewer.
function UnifiedDiff({ text }: { text: string }) {
  const lines = text.split("\n");
  return (
    <pre className="edit-diff" style={{ borderLeftColor: "var(--border)" }}>
      {lines.map((line, i) => {
        let cls = "udiff-line";
        if (line.startsWith("+++") || line.startsWith("---")) cls += " udiff-meta";
        else if (line.startsWith("@@")) cls += " udiff-hunk";
        else if (line.startsWith("Index:") || line.startsWith("====")) cls += " udiff-meta";
        else if (line.startsWith("+")) cls += " udiff-add";
        else if (line.startsWith("-")) cls += " udiff-rem";
        return (
          <span key={i} className={cls}>
            {line.length === 0 ? "\u00A0" : line}
            {i < lines.length - 1 ? "\n" : ""}
          </span>
        );
      })}
    </pre>
  );
}

// ---------------------------------------------------------------------------
// Inline line-by-line diff for the `edit` tool's old_str → new_str change.
// Renders a single unified column (git-style: removed lines then added lines)
// with:
//   - a left gutter of old/new line numbers plus a -/+ change marker
//   - Prism syntax highlighting per line (foreground token colors)
//   - intra-line word-level emphasis (diffWordsWithSpace) on paired
//     removed/added lines so the exact changed words stand out
// ---------------------------------------------------------------------------

// Above these limits we fall back to cheaper rendering to keep large
// whole-file rewrites from freezing the UI.
const MAX_DIFF_ROWS = 4000;
const MAX_WORD_DIFF_LINE = 2000;

type DiffRow = {
  kind: "add" | "rem" | "eq";
  oldNo: number | null;
  newNo: number | null;
  text: string;
  // Per-character "changed" mask (UTF-16 code units) for paired rows.
  // Absent on equal rows and on unpaired add/rem rows (the row tint alone
  // signals those).
  mask?: boolean[];
};

// Split a diff chunk into display lines. Handles CRLF/CR and drops only the
// single trailing empty element produced by a terminating newline, while
// preserving real interior blank lines.
function splitChunkLines(text: string): string[] {
  const parts = text.split(/\r\n|\n|\r/);
  if (parts.length > 1 && parts[parts.length - 1] === "") parts.pop();
  return parts;
}

function buildDiffRows(oldStr: string, newStr: string): DiffRow[] {
  const changes = diffLines(oldStr, newStr);
  const rows: DiffRow[] = [];
  let oldNo = 1;
  let newNo = 1;
  for (const ch of changes) {
    for (const line of splitChunkLines(ch.value)) {
      if (ch.added) {
        rows.push({ kind: "add", oldNo: null, newNo: newNo++, text: line });
      } else if (ch.removed) {
        rows.push({ kind: "rem", oldNo: oldNo++, newNo: null, text: line });
      } else {
        rows.push({ kind: "eq", oldNo: oldNo++, newNo: newNo++, text: line });
      }
    }
  }
  return rows;
}

// Build per-character changed masks for a removed/added line pair via a
// word-level diff. Returns null when either line is too long to bother.
function computeWordMasks(
  remText: string,
  addText: string,
): { rem: boolean[]; add: boolean[] } | null {
  if (remText.length > MAX_WORD_DIFF_LINE || addText.length > MAX_WORD_DIFF_LINE) {
    return null;
  }
  const segs = diffWordsWithSpace(remText, addText);
  const rem: boolean[] = [];
  const add: boolean[] = [];
  for (const s of segs) {
    const len = s.value.length;
    if (s.added) {
      for (let i = 0; i < len; i++) add.push(true);
    } else if (s.removed) {
      for (let i = 0; i < len; i++) rem.push(true);
    } else {
      for (let i = 0; i < len; i++) {
        rem.push(false);
        add.push(false);
      }
    }
  }
  return { rem, add };
}

// Walk the rows and, for each removed block immediately followed by an added
// block, pair lines positionally (rem[i] ↔ add[i]) and attach word-level
// masks. Leftover lines from unequal block lengths stay fully highlighted by
// the row tint (no mask).
function applyWordMasks(rows: DiffRow[]): void {
  let i = 0;
  while (i < rows.length) {
    if (rows[i].kind !== "rem") {
      i++;
      continue;
    }
    let r = i;
    while (r < rows.length && rows[r].kind === "rem") r++;
    if (r >= rows.length || rows[r].kind !== "add") {
      i = r;
      continue;
    }
    let a = r;
    while (a < rows.length && rows[a].kind === "add") a++;
    const pairs = Math.min(r - i, a - r);
    for (let k = 0; k < pairs; k++) {
      const remRow = rows[i + k];
      const addRow = rows[r + k];
      const masks = computeWordMasks(remRow.text, addRow.text);
      if (masks) {
        remRow.mask = masks.rem;
        addRow.mask = masks.add;
      }
    }
    i = a;
  }
}

// Build the Prism CSS class string for a token, mirroring Prism's own
// stringify output (`token <type> <aliases…>`).
function tokenClass(type: string, alias: string | string[] | undefined): string {
  const parts = ["token", type];
  if (alias) {
    if (Array.isArray(alias)) parts.push(...alias);
    else parts.push(alias);
  }
  return parts.join(" ");
}

// Flatten Prism's nested token stream into a flat list of styled segments
// (text + className), prepending parent token classes onto nested children so
// each leaf carries its full styling context.
function flattenTokens(
  tokens: Array<string | Prism.Token>,
  baseCls: string,
  out: Array<{ text: string; cls: string }>,
): void {
  for (const tok of tokens) {
    if (typeof tok === "string") {
      if (tok) out.push({ text: tok, cls: baseCls });
      continue;
    }
    const c = tokenClass(tok.type, tok.alias);
    const merged = baseCls ? `${baseCls} ${c}` : c;
    const content = tok.content;
    if (typeof content === "string") {
      if (content) out.push({ text: content, cls: merged });
    } else {
      const arr = Array.isArray(content) ? content : [content];
      flattenTokens(arr as Array<string | Prism.Token>, merged, out);
    }
  }
}

// Produce the styled segments for one line. With a grammar, syntax-highlight
// via Prism; otherwise the whole line is a single unstyled segment.
function styleLine(
  text: string,
  grammar: Prism.Grammar | null,
  language: string | null,
): Array<{ text: string; cls: string }> {
  if (!grammar || !language) return text.length ? [{ text, cls: "" }] : [];
  const out: Array<{ text: string; cls: string }> = [];
  flattenTokens(Prism.tokenize(text, grammar), "", out);
  return out;
}

// Render a line's styled segments as React spans, splitting each segment at
// changed/unchanged boundaries (from the word mask) so changed words get an
// extra emphasis background on top of their syntax color.
function renderStyledLine(
  segs: Array<{ text: string; cls: string }>,
  mask: boolean[] | undefined,
): ReactNode {
  if (segs.length === 0) return "\u00A0";
  const nodes: ReactNode[] = [];
  let pos = 0;
  let key = 0;
  for (const seg of segs) {
    const { text, cls } = seg;
    let i = 0;
    while (i < text.length) {
      const changed = mask ? !!mask[pos + i] : false;
      let j = i + 1;
      while (j < text.length && (mask ? !!mask[pos + j] : false) === changed) j++;
      const piece = text.slice(i, j);
      const full = [cls, changed ? "idiff-ch" : ""].filter(Boolean).join(" ");
      nodes.push(
        full ? (
          <span key={key++} className={full}>{piece}</span>
        ) : (
          <span key={key++}>{piece}</span>
        ),
      );
      i = j;
    }
    pos += text.length;
  }
  return nodes;
}

function InlineDiff({
  oldStr,
  newStr,
  language,
  externalQuery,
}: {
  oldStr: string;
  newStr: string;
  language: string | null;
  externalQuery?: string;
}) {
  const prepared = useMemo(() => {
    const rows = buildDiffRows(oldStr, newStr);
    // For very large diffs, skip the expensive word-level masks and Prism
    // tokenizing — keep only line-level coloring + line numbers.
    const degraded = rows.length > MAX_DIFF_ROWS;
    if (!degraded) applyWordMasks(rows);
    const grammar = !degraded && language ? Prism.languages[language] ?? null : null;
    const lang = degraded ? null : language;
    return rows.map((row) => ({
      row,
      nodes: renderStyledLine(styleLine(row.text, grammar, lang), degraded ? undefined : row.mask),
    }));
  }, [oldStr, newStr, language]);

  return (
    <TextBlock searchable externalQuery={externalQuery}>
      <div className="inline-diff">
        {prepared.map(({ row, nodes }, i) => (
          <div key={i} className={`idiff-row idiff-${row.kind}`}>
            <span className="idiff-no">{row.oldNo ?? ""}</span>
            <span className="idiff-no">{row.newNo ?? ""}</span>
            <span className="idiff-mark">
              {row.kind === "add" ? "+" : row.kind === "rem" ? "-" : "\u00A0"}
            </span>
            <span className="idiff-code">{nodes}</span>
          </div>
        ))}
      </div>
    </TextBlock>
  );
}

// Specialized view for file-reading tools (`view`, `read`). Renders
//   - path           plain (accepts `path` or `filePath`)
//   - any other args fall through as JSON (e.g. view_range, limit, offset)
//   - result         syntax-highlighted using the path's extension
//
// Result-shape handling:
//   * Copilot `view` returns a raw string body with `N. ` line prefixes.
//   * opencode `read` returns a JSON envelope `{title, output, metadata?}`
//     whose `output` wraps the body in `<path>…</path>\n<type>file</type>\n
//     <content>\n…\n</content>` with `N: ` line prefixes.
// We unwrap the envelope and the XML wrapper when present, then strip
// either prefix style into a left-gutter.
function ViewArgs({ attributes, externalQuery }: { attributes: Record<string, unknown>; externalQuery?: string }) {
  const args = parseToolCallArguments(attributes);
  const result = parseToolCallResult(attributes);
  const argsObj =
    args && typeof args === "object" && !Array.isArray(args)
      ? (args as Record<string, unknown>)
      : null;
  if (!argsObj && result == null) return <div className="no-content">{NO_CONTENT_LINE}</div>;
  const path = argsObj && typeof argsObj.path === "string"
    ? argsObj.path
    : argsObj && typeof argsObj.filePath === "string"
      ? argsObj.filePath
      : null;
  const lang = langFromPath(path);
  const extraEntries = argsObj
    ? Object.entries(argsObj).filter(([k]) => k !== "path" && k !== "filePath")
    : [];
  const extraObj = extraEntries.length ? Object.fromEntries(extraEntries) : null;
  // Resolve the file body from the result. Copilot returns a string
  // directly; opencode returns `{output: string, …}` where `output`
  // may further wrap the body in `<path>/<type>/<content>` XML tags.
  let body: string | null = null;
  if (typeof result === "string") {
    body = result;
  } else if (result && typeof result === "object" && !Array.isArray(result)) {
    const out = (result as Record<string, unknown>).output;
    if (typeof out === "string") body = out;
  }
  if (body != null) {
    const wrapper = /^<path>[^<]*<\/path>\s*<type>[^<]*<\/type>\s*<content>\s*([\s\S]*?)\s*<\/content>\s*$/;
    const m = wrapper.exec(body);
    if (m) body = m[1];
  }
  // The `view`/`read` tools prepend line-number prefixes (`N. ` for
  // Copilot, `N: ` for opencode) to every line of the file body. Split
  // those off before handing the content to Prism so the highlighter
  // sees real source code, then re-render the line numbers in a dim
  // left gutter via a 2-column grid.
  let lns: string | null = null;
  if (body != null) {
    const lines = body.split("\n");
    const prefixes: string[] = [];
    const stripped: string[] = [];
    for (const line of lines) {
      const m = /^(\d+)[.:]\s(.*)$/.exec(line);
      if (m) {
        prefixes.push(m[1]);
        stripped.push(m[2]);
      } else {
        prefixes.push("");
        stripped.push(line);
      }
    }
    if (prefixes.some((p) => p)) {
      lns = prefixes.join("\n");
      body = stripped.join("\n");
    }
  }
  return (
    <>
      {argsObj && (
        <div className="shell">
          <div className="label">arguments</div>
          {path != null && (
            <div className="kv" style={{ padding: "2px 0" }}>
              <span className="k">path</span>
              <span className="v mono">{path}</span>
            </div>
          )}
          {extraObj && (
            <>
              <div className="label" style={{ marginTop: 4 }}>other</div>
              <TextBlock searchable text={prettyJson(extraObj)} preClassName="json" externalQuery={externalQuery} />
            </>
          )}
        </div>
      )}
      {result != null && (
        <div className="shell">
          <div className="label">result</div>
          {body != null ? (
            lns != null ? (
              <div className="lineno-block edit-diff">
                <pre className="lns">{lns}</pre>
                <TextBlock searchable externalQuery={externalQuery}>
                  <CodeBlock language={lang} text={body} />
                </TextBlock>
              </div>
            ) : (
              <TextBlock searchable externalQuery={externalQuery}>
                <CodeBlock language={lang} text={body} className="edit-diff" />
              </TextBlock>
            )
          ) : (
            <TextBlock searchable text={prettyJson(result)} preClassName="json" externalQuery={externalQuery} />
          )}
        </div>
      )}
    </>
  );
}

// Specialized view for the `task` sub-agent dispatcher. The `prompt`
// argument is markdown (model instructions), so render it via
// react-markdown; render the remaining scalar args as a plain kv list,
// and pass the result through as markdown when it's a string.
function TaskArgs({ attributes, externalQuery }: { attributes: Record<string, unknown>; externalQuery?: string }) {
  const args = parseToolCallArguments(attributes);
  const result = parseToolCallResult(attributes);
  const argsObj =
    args && typeof args === "object" && !Array.isArray(args)
      ? (args as Record<string, unknown>)
      : null;
  if (args == null && result == null) return <div className="no-content">{NO_CONTENT_LINE}</div>;
  let prompt: string | null = null;
  let otherArgs: Array<[string, unknown]> = [];
  if (argsObj) {
    for (const [k, v] of Object.entries(argsObj)) {
      if (k === "prompt" && typeof v === "string") prompt = v;
      else otherArgs.push([k, v]);
    }
  }
  return (
    <>
      {args != null && (
        <div className="shell">
          <div className="label">arguments</div>
          {argsObj ? (
            <>
              {otherArgs.length > 0 && (
                <div className="kv" style={{ padding: "2px 0" }}>
                  {otherArgs.map(([k, v]) => (
                    <span key={k} style={{ display: "contents" }}>
                      <span className="k">{k}</span>
                      <span className="v mono">
                        {typeof v === "string" ? v : prettyJson(v)}
                      </span>
                    </span>
                  ))}
                </div>
              )}
              {prompt != null && (
                <>
                  <div className="label" style={{ marginTop: 6 }}>prompt</div>
                  <TextBlock searchable externalQuery={externalQuery}>
                    <div className="markdown-body">
                      <ReactMarkdown remarkPlugins={[remarkGfm]}>{prompt}</ReactMarkdown>
                    </div>
                  </TextBlock>
                </>
              )}
            </>
          ) : (
            <TextBlock searchable text={prettyJson(args)} preClassName="json" externalQuery={externalQuery} />
          )}
        </div>
      )}
      {result != null && (
        <div className="shell">
          <div className="label">result</div>
          {typeof result === "string" ? (
            <TextBlock searchable externalQuery={externalQuery}>
              <div className="markdown-body">
                <ReactMarkdown remarkPlugins={[remarkGfm]}>{result}</ReactMarkdown>
              </div>
            </TextBlock>
          ) : (
            <TextBlock searchable text={prettyJson(result)} preClassName="json" externalQuery={externalQuery} />
          )}
        </div>
      )}
    </>
  );
}

// Specialized view for the `read_agent` sub-agent inspector. Renders
//   - args           plain kv (agent_id, wait, timeout, since_turn)
//   - result         react-markdown (GFM) when the result is a string;
//                    falls back to JSON otherwise
function ReadAgentArgs({ attributes, externalQuery }: { attributes: Record<string, unknown>; externalQuery?: string }) {
  const args = parseToolCallArguments(attributes);
  const result = parseToolCallResult(attributes);
  const argsObj =
    args && typeof args === "object" && !Array.isArray(args)
      ? (args as Record<string, unknown>)
      : null;
  if (args == null && result == null) return <div className="no-content">{NO_CONTENT_LINE}</div>;
  return (
    <>
      {args != null && (
        <div className="shell">
          <div className="label">arguments</div>
          {argsObj ? (
            <div className="kv" style={{ padding: "2px 0" }}>
              {Object.entries(argsObj).map(([k, v]) => (
                <span key={k} style={{ display: "contents" }}>
                  <span className="k">{k}</span>
                  <span className="v mono">
                    {typeof v === "string" ? v : prettyJson(v)}
                  </span>
                </span>
              ))}
            </div>
          ) : (
            <TextBlock searchable text={prettyJson(args)} preClassName="json" externalQuery={externalQuery} />
          )}
        </div>
      )}
      {result != null && (
        <div className="shell">
          <div className="label">result</div>
          {typeof result === "string" ? (
            <TextBlock searchable externalQuery={externalQuery}>
              <div className="markdown-body">
                <ReactMarkdown remarkPlugins={[remarkGfm]}>{result}</ReactMarkdown>
              </div>
            </TextBlock>
          ) : (
            <TextBlock searchable text={prettyJson(result)} preClassName="json" externalQuery={externalQuery} />
          )}
        </div>
      )}
    </>
  );
}
