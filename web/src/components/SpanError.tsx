import type { Nullable } from "../api/types";

export interface SpanErrorFields {
  error_type?: Nullable<string>;
  status_code?: Nullable<number>;
}

export function isErrorStatusCode(statusCode: Nullable<number> | undefined): boolean {
  return statusCode != null && statusCode !== 0 && statusCode !== 1;
}

export function hasSpanError(span: SpanErrorFields): boolean {
  return span.error_type != null || isErrorStatusCode(span.status_code);
}

function errorParts(span: SpanErrorFields): string[] {
  const parts: string[] = [];
  if (span.error_type != null) parts.push(`error.type: ${span.error_type || "—"}`);
  if (isErrorStatusCode(span.status_code)) parts.push(`status_code: ${span.status_code}`);
  return parts;
}

export function SpanErrorIndicator({ span }: { span: SpanErrorFields }) {
  if (!hasSpanError(span)) return null;
  return (
    <span className="tag err" data-tip={errorParts(span).join(" · ") || "span error"}>
      err
    </span>
  );
}

export function SpanErrorBlock({ span }: { span: SpanErrorFields }) {
  if (!hasSpanError(span)) return null;
  return (
    <div className="span-error" role="alert">
      <span className="tag err">error</span>
      <div className="kv span-error-kv">
        {span.error_type != null && (
          <>
            <span className="k">error.type</span>
            <span className="v mono">{span.error_type || "—"}</span>
          </>
        )}
        {isErrorStatusCode(span.status_code) && (
          <>
            <span className="k">status_code</span>
            <span className="v mono">{span.status_code}</span>
          </>
        )}
      </div>
    </div>
  );
}
