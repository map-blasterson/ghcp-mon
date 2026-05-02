import { useState, type ReactNode } from "react";

export interface TextBlockProps {
  text?: string;
  children?: ReactNode;
  className?: string;
  preClassName?: string;
  truncatable?: boolean;
  searchable?: boolean;
}

export function TextBlock({
  text,
  children,
  className,
  preClassName,
  truncatable = false,
  searchable = true,
}: TextBlockProps) {
  // TODO(phase2): wire `searchable` to ?/ hover icon + inline search input.
  void searchable;

  const [open, setOpen] = useState(false);

  const isLong =
    truncatable && typeof text === "string" &&
    (text.length > 200 || text.includes("\n"));

  const body = text !== undefined ? (
    <pre
      className={`${preClassName ?? ""}${isLong ? " ib-prim-v-clip" : ""}${isLong && open ? " open" : ""}`.trim()}
    >
      {text}
    </pre>
  ) : (
    children
  );

  const wrapperClass = `tb-block${className ? " " + className : ""}`;

  return (
    <div className={wrapperClass}>
      {isLong && (
        <button
          type="button"
          className="tb-expand-hit"
          aria-label={open ? "Collapse" : "Expand"}
          aria-expanded={open}
          onClick={(e) => {
            e.stopPropagation();
            setOpen((v) => !v);
          }}
        >
          <span className="ib-caret">{open ? "▾" : "▸"}</span>
        </button>
      )}
      {body}
    </div>
  );
}

export default TextBlock;
