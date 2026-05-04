import { useQuery } from "@tanstack/react-query";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
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

function ToolDetailBody({ detail, externalQuery }: { detail: SpanDetail; externalQuery?: string }) {
  const tc = detail.projection.tool_call!;
  const span = detail.span;
  const dur =
    span.duration_ns ??
    (span.start_unix_ns != null && span.end_unix_ns != null
      ? span.end_unix_ns - span.start_unix_ns
      : null);
  const a = span.attributes ?? {};
  return (
    <>
      <div className="section">
        <h4>{tc.tool_name ?? "(unknown tool)"}</h4>
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
      </div>
      <div className="section">
        <h4>args / result</h4>
        {tc.tool_name === "edit" ? (
          <EditArgs attributes={a} externalQuery={externalQuery} />
        ) : tc.tool_name === "view" ? (
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
      <div className="section">
        <h4>{ext.tool_name ?? "(unknown tool)"}</h4>
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
      </div>
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

// Specialized view for the `edit` file-edit tool. Renders
//   - path           plain
//   - old_str        red, with newlines preserved verbatim
//   - new_str        green, with newlines preserved verbatim
//   - any other args fall through as JSON
//   - result         same fallback as GenericArgs
function EditArgs({ attributes, externalQuery }: { attributes: Record<string, unknown>; externalQuery?: string }) {
  const args = parseToolCallArguments(attributes);
  const result = parseToolCallResult(attributes);
  const argsObj =
    args && typeof args === "object" && !Array.isArray(args)
      ? (args as Record<string, unknown>)
      : null;
  if (!argsObj && result == null) return <div className="no-content">{NO_CONTENT_LINE}</div>;
  const path = argsObj && typeof argsObj.path === "string" ? argsObj.path : null;
  const oldStr = argsObj && typeof argsObj.old_str === "string" ? argsObj.old_str : null;
  const newStr = argsObj && typeof argsObj.new_str === "string" ? argsObj.new_str : null;
  const lang = langFromPath(path);
  const extraEntries = argsObj
    ? Object.entries(argsObj).filter(
        ([k]) => k !== "path" && k !== "old_str" && k !== "new_str"
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
              <div className="label" style={{ marginTop: 4 }}>new_str</div>
              <TextBlock searchable externalQuery={externalQuery}>
                <CodeBlock
                  language={lang}
                  text={newStr}
                  className="edit-diff edit-diff-new"
                />
              </TextBlock>
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

// Specialized view for the `view` file-read tool. Renders
//   - path           plain
//   - any other args fall through as JSON (e.g. view_range)
//   - result         syntax-highlighted using the path's extension
function ViewArgs({ attributes, externalQuery }: { attributes: Record<string, unknown>; externalQuery?: string }) {
  const args = parseToolCallArguments(attributes);
  const result = parseToolCallResult(attributes);
  const argsObj =
    args && typeof args === "object" && !Array.isArray(args)
      ? (args as Record<string, unknown>)
      : null;
  if (!argsObj && result == null) return <div className="no-content">{NO_CONTENT_LINE}</div>;
  const path = argsObj && typeof argsObj.path === "string" ? argsObj.path : null;
  const lang = langFromPath(path);
  const extraEntries = argsObj
    ? Object.entries(argsObj).filter(([k]) => k !== "path")
    : [];
  const extraObj = extraEntries.length ? Object.fromEntries(extraEntries) : null;
  // The `view` tool prepends 'N. ' line numbers to every line of the
  // file body. We split those off before handing the content to Prism
  // so the highlighter sees real source code, then re-render the line
  // numbers in a dim left gutter via a 2-column grid.
  let lns: string | null = null;
  let body: string | null = typeof result === "string" ? result : null;
  if (body != null) {
    const lines = body.split("\n");
    const prefixes: string[] = [];
    const stripped: string[] = [];
    for (const line of lines) {
      const m = /^(\d+)\.\s(.*)$/.exec(line);
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
