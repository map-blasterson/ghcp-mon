interface Props {
  value: unknown;
  collapsed?: boolean;
}

export function JsonView({ value, collapsed }: Props) {
  if (collapsed) {
    return (
      <details>
        <summary>json…</summary>
        <pre className="json">{stringify(value)}</pre>
      </details>
    );
  }
  return <pre className="json">{stringify(value)}</pre>;
}

function stringify(v: unknown): string {
  try {
    return JSON.stringify(v, null, 2);
  } catch {
    return String(v);
  }
}
