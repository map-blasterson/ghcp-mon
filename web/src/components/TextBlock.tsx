import {
  useCallback,
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
  type ReactNode,
} from "react";

export interface TextBlockProps {
  text?: string;
  children?: ReactNode;
  className?: string;
  preClassName?: string;
  truncatable?: boolean;
  searchable?: boolean;
  // Controlled expand/collapse for truncatable mode. The chevron affordance
  // was removed — callers drive open/close from elsewhere (e.g. ChatDetail
  // toggles via clicks on the .ib-prim-k key span). When `open` is omitted
  // the block stays collapsed.
  open?: boolean;
}

type SearchPhase = "idle" | "icon" | "active";

const ICON_OFFSET = 12;

// Unwrap any <mark class="tb-match"> children inside `root`, restoring the
// original text-node layout so the TreeWalker can re-walk fresh content.
function unwrapMarks(root: HTMLElement) {
  const marks = root.querySelectorAll("mark.tb-match");
  marks.forEach((m) => {
    const parent = m.parentNode;
    if (!parent) return;
    while (m.firstChild) parent.insertBefore(m.firstChild, m);
    parent.removeChild(m);
    parent.normalize();
  });
}

// Walk text nodes under `root` and wrap each case-insensitive occurrence of
// `query` in a <mark class="tb-match">. Returns the list of created marks in
// document order.
function wrapMatches(root: HTMLElement, query: string): HTMLElement[] {
  if (!query) return [];
  const q = query.toLowerCase();
  const walker = document.createTreeWalker(root, NodeFilter.SHOW_TEXT, {
    acceptNode(node) {
      // Skip text inside our own header / icon overlays, and inside any
      // existing <mark> (we already unwrapped, but be defensive).
      let p: Node | null = node.parentNode;
      while (p && p !== root) {
        if (p instanceof HTMLElement) {
          if (
            p.classList.contains("tb-search-header") ||
            p.classList.contains("tb-search-icon") ||
            p.classList.contains("tb-search-input") ||
            p.tagName === "MARK"
          ) {
            return NodeFilter.FILTER_REJECT;
          }
        }
        p = p.parentNode;
      }
      return NodeFilter.FILTER_ACCEPT;
    },
  });
  const targets: Text[] = [];
  let n: Node | null;
  while ((n = walker.nextNode())) {
    if (n.nodeValue && n.nodeValue.toLowerCase().includes(q)) {
      targets.push(n as Text);
    }
  }
  const created: HTMLElement[] = [];
  for (const tn of targets) {
    const value = tn.nodeValue ?? "";
    const lower = value.toLowerCase();
    let cursor = 0;
    let idx = lower.indexOf(q, cursor);
    if (idx < 0) continue;
    const parent = tn.parentNode;
    if (!parent) continue;
    const frag = document.createDocumentFragment();
    while (idx >= 0) {
      if (idx > cursor) {
        frag.appendChild(document.createTextNode(value.slice(cursor, idx)));
      }
      const mark = document.createElement("mark");
      mark.className = "tb-match";
      mark.appendChild(document.createTextNode(value.slice(idx, idx + q.length)));
      frag.appendChild(mark);
      created.push(mark);
      cursor = idx + q.length;
      idx = lower.indexOf(q, cursor);
    }
    if (cursor < value.length) {
      frag.appendChild(document.createTextNode(value.slice(cursor)));
    }
    parent.replaceChild(frag, tn);
  }
  return created;
}

export function TextBlock({
  text,
  children,
  className,
  preClassName,
  truncatable = false,
  searchable = true,
  open,
}: TextBlockProps) {
  // Search state machine.
  const [phase, setPhase] = useState<SearchPhase>("idle");
  const [query, setQuery] = useState("");
  const [matchIndex, setMatchIndex] = useState(0);
  const [matchCount, setMatchCount] = useState(0);

  const wrapperRef = useRef<HTMLDivElement | null>(null);
  const contentRef = useRef<HTMLDivElement | null>(null);
  const iconRef = useRef<HTMLSpanElement | null>(null);
  const inputWrapRef = useRef<HTMLDivElement | null>(null);
  const inputRef = useRef<HTMLInputElement | null>(null);
  // Last mouse position relative to the wrapper, kept in a ref so we can
  // seed a freshly-mounted target (e.g. the search input on click) with an
  // initial transform — otherwise it flashes at translate(0,0) (the
  // header) until the next mousemove.
  const lastMouseRef = useRef<{ x: number; y: number } | null>(null);

  const isLong =
    truncatable && typeof text === "string" &&
    (text.length > 200 || text.includes("\n"));
  const effectiveOpen = !!open;

  const body = text !== undefined ? (
    <pre
      className={`${preClassName ?? ""}${isLong ? " ib-prim-v-clip" : ""}${isLong && effectiveOpen ? " open" : ""}`.trim()}
    >
      {text}
    </pre>
  ) : (
    children
  );

  const wrapperClass = `tb-block${className ? " " + className : ""}${phase === "active" ? " tb-active" : ""}`;

  // ---- cursor tracking -----------------------------------------------------
  // While the block is hovered (icon phase) or search is active, position the
  // ?/ icon / input via imperative transform updates so we don't re-render on
  // every mousemove.
  useEffect(() => {
    if (!searchable) return;
    if (phase === "idle") return;
    const wrap = wrapperRef.current;
    if (!wrap) return;

    const target = phase === "active" ? inputWrapRef.current : iconRef.current;
    if (!target) return;

    // Seed the new target with the last known cursor position so it doesn't
    // flash at translate(0,0) (the header) until the first mousemove.
    if (lastMouseRef.current) {
      const { x, y } = lastMouseRef.current;
      target.style.transform = `translate(${x}px, ${y}px)`;
    }

    const onMove = (e: MouseEvent) => {
      const rect = wrap.getBoundingClientRect();
      const x = e.clientX - rect.left + ICON_OFFSET;
      const y = e.clientY - rect.top + ICON_OFFSET;
      lastMouseRef.current = { x, y };
      target.style.transform = `translate(${x}px, ${y}px)`;
    };
    wrap.addEventListener("mousemove", onMove);
    return () => {
      wrap.removeEventListener("mousemove", onMove);
    };
  }, [phase, searchable]);

  // ---- hover enter/leave wiring (icon phase) ------------------------------
  const onMouseEnter = useCallback(() => {
    if (!searchable) return;
    setPhase((p) => (p === "idle" ? "icon" : p));
  }, [searchable]);

  const onMouseLeave = useCallback(() => {
    setPhase((p) => (p === "icon" ? "idle" : p));
  }, []);

  // ---- exit wiring: column mouseleave, ESC, right-click -------------------
  const exitSearch = useCallback(() => {
    // If the cursor is still over the block, hop straight back to the
    // icon phase so the ?/ glyph reappears immediately. Otherwise the
    // user has to leave the block and re-enter to reactivate it because
    // mouseenter doesn't refire while the cursor is already inside.
    const stillHovering = !!wrapperRef.current?.matches(":hover");
    setPhase(stillHovering ? "icon" : "idle");
    setQuery("");
    setMatchIndex(0);
    setMatchCount(0);
  }, []);

  useEffect(() => {
    if (phase !== "active") return;
    const wrap = wrapperRef.current;
    if (!wrap) return;
    const colBody = wrap.closest(".col-body") as HTMLElement | null;
    if (!colBody) return;
    const onLeave = () => exitSearch();
    colBody.addEventListener("mouseleave", onLeave);
    return () => colBody.removeEventListener("mouseleave", onLeave);
  }, [phase, exitSearch]);

  useEffect(() => {
    if (phase !== "active") return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.preventDefault();
        exitSearch();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [phase, exitSearch]);

  // ---- match-cycling click handler on the block ---------------------------
  const onBlockClick = useCallback(
    (e: React.MouseEvent) => {
      if (!searchable) return;
      // Icon phase: any click on the block activates search mode. The
      // floating ?/ glyph is decorative (pointer-events: none) since it
      // tracks 12px ahead of the cursor and would otherwise be uncatchable.
      if (phase === "icon") {
        e.preventDefault();
        e.stopPropagation();
        setPhase("active");
        return;
      }
      if (phase !== "active") return;
      // Don't intercept clicks on the input itself.
      const tgt = e.target as HTMLElement | null;
      if (tgt && tgt.closest(".tb-search-input-wrap")) return;
      if (matchCount === 0) return;
      e.preventDefault();
      e.stopPropagation();
      setMatchIndex((i) => {
        if (e.shiftKey) return (i - 1 + matchCount) % matchCount;
        return (i + 1) % matchCount;
      });
    },
    [phase, matchCount, searchable],
  );

  const onBlockContextMenu = useCallback(
    (e: React.MouseEvent) => {
      if (phase !== "active") return;
      e.preventDefault();
      exitSearch();
    },
    [phase, exitSearch],
  );

  // ---- DOM highlight effect (debounced 50ms) ------------------------------
  useLayoutEffect(() => {
    if (!searchable) return;
    const root = contentRef.current;
    if (!root) return;

    // Always unwrap previous marks first, even when going idle, so we don't
    // leak <mark> wrappers into the DOM on cleanup or query change.
    unwrapMarks(root);

    if (phase !== "active" || !query) {
      setMatchCount(0);
      return;
    }

    let cancelled = false;
    const timer = window.setTimeout(() => {
      if (cancelled) return;
      const r = contentRef.current;
      if (!r) return;
      // Unwrap again in case a re-render added new content since the timer
      // was scheduled.
      unwrapMarks(r);
      const marks = wrapMatches(r, query);
      const count = marks.length;
      const idx = count === 0 ? 0 : Math.min(matchIndex, count - 1);
      if (count > 0) {
        marks[idx].classList.add("tb-match-current");
        marks[idx].scrollIntoView({ block: "nearest" });
      }
      setMatchCount(count);
      if (idx !== matchIndex) setMatchIndex(idx);
    }, 50);

    return () => {
      cancelled = true;
      window.clearTimeout(timer);
      const r = contentRef.current;
      if (r) unwrapMarks(r);
    };
  }, [phase, query, matchIndex, searchable]);

  // ---- focus the input on activation --------------------------------------
  useEffect(() => {
    if (phase === "active") {
      inputRef.current?.focus();
    }
  }, [phase]);

  const showHover = searchable && (phase === "icon" || phase === "active");

  const headerRight =
    matchCount > 0 ? "(shift)+LMB (prev)/(next)" : "(shift)+LMB (prev)/(next)";
  const headerLeft =
    !query || matchCount === 0
      ? "0 matches"
      : `${matchIndex + 1} of ${matchCount} matches`;

  return (
    <div
      ref={wrapperRef}
      className={wrapperClass}
      onMouseEnter={onMouseEnter}
      onMouseLeave={onMouseLeave}
      onClick={onBlockClick}
      onContextMenu={onBlockContextMenu}
    >
      {phase === "active" && (
        <div className="tb-search-header" aria-hidden="true">
          <span className="tb-search-header-l">{headerLeft}</span>
          <span className="tb-search-header-r">{headerRight}</span>
        </div>
      )}
      <div ref={contentRef} className="tb-content">
        {body}
      </div>
      {showHover && phase === "icon" && (
        <span
          ref={iconRef}
          className="tb-search-icon"
          aria-hidden="true"
        >
          ?/
        </span>
      )}
      {phase === "active" && (
        <div ref={inputWrapRef} className="tb-search-input-wrap">
          <span className="tb-search-input-prefix">?/</span>
          <input
            ref={inputRef}
            type="search"
            role="searchbox"
            aria-label="search in block"
            className="tb-search-input"
            value={query}
            onChange={(e) => {
              setQuery(e.target.value);
              setMatchIndex(0);
            }}
            onClick={(e) => e.stopPropagation()}
            onContextMenu={(e) => {
              e.preventDefault();
              e.stopPropagation();
              exitSearch();
            }}
            onKeyDown={(e) => {
              if (e.key === "Escape") {
                e.preventDefault();
                exitSearch();
              } else if (e.key === "Enter") {
                e.preventDefault();
                if (matchCount > 0) {
                  setMatchIndex((i) =>
                    e.shiftKey
                      ? (i - 1 + matchCount) % matchCount
                      : (i + 1) % matchCount,
                  );
                }
              }
            }}
          />
        </div>
      )}
    </div>
  );
}

export default TextBlock;
