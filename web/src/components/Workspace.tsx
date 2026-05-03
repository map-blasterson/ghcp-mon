import { useCallback, useLayoutEffect, useMemo, useRef, useState } from "react";
import { useWorkspace } from "../state/workspace";
import { ColumnBody } from "./Column";

const RESIZER_PX = 4;
const DEFAULT_MIN_COL_PX = 280;

export function Workspace() {
  const columns = useWorkspace((s) => s.columns);
  const updateColumn = useWorkspace((s) => s.updateColumn);
  const wsRef = useRef<HTMLDivElement>(null);
  const columnRefs = useRef(new Map<string, HTMLDivElement>());
  const [containerPx, setContainerPx] = useState(0);
  const [naturalMinPxById, setNaturalMinPxById] = useState<Record<string, number>>({});
  const columnIdKey = columns.map((c) => c.id).join("\0");
  const columnIds = useMemo(() => columns.map((c) => c.id), [columnIdKey]);

  // Track container width via ResizeObserver so layout adapts to window /
  // panel resizes precisely, with no fr-unit sub-pixel drift.
  useLayoutEffect(() => {
    const el = wsRef.current;
    if (!el) return;
    setContainerPx(el.clientWidth);
    const ro = new ResizeObserver((entries) => {
      for (const e of entries) setContainerPx(e.contentRect.width);
    });
    ro.observe(el);
    return () => ro.disconnect();
  }, []);

  const setNaturalMin = useCallback((id: string, px: number) => {
    setNaturalMinPxById((prev) => (prev[id] === px ? prev : { ...prev, [id]: px }));
  }, []);

  const measureNaturalMins = useCallback(() => {
    const next: Record<string, number> = {};
    for (const id of columnIds) {
      const el = columnRefs.current.get(id);
      if (el) next[id] = measureColumnMinPx(el);
    }
    setNaturalMinPxById((prev) => {
      const prevKeys = Object.keys(prev);
      const nextKeys = Object.keys(next);
      if (
        prevKeys.length === nextKeys.length &&
        nextKeys.every((key) => prev[key] === next[key])
      ) {
        return prev;
      }
      return next;
    });
  }, [columnIds]);

  // Keep the JS layout model in sync with the browser's intrinsic min-content
  // width for each column, including content that changes after initial render.
  useLayoutEffect(() => {
    let frame = 0;
    const scheduleMeasure = () => {
      cancelAnimationFrame(frame);
      frame = requestAnimationFrame(measureNaturalMins);
    };

    const ro = new ResizeObserver(scheduleMeasure);
    const mo = new MutationObserver(scheduleMeasure);

    for (const id of columnIds) {
      const el = columnRefs.current.get(id);
      if (!el) continue;
      ro.observe(el);
      mo.observe(el, {
        attributes: true,
        childList: true,
        characterData: true,
        subtree: true,
      });
    }

    scheduleMeasure();
    return () => {
      cancelAnimationFrame(frame);
      ro.disconnect();
      mo.disconnect();
    };
  }, [columnIds, measureNaturalMins]);

  if (columns.length === 0) {
    return <div className="empty-state">no columns. add one from the top bar.</div>;
  }

  // Distribute the available pixel width by weight (the persisted "width"
  // value, treated as a relative weight). Min-clamp each column to its
  // measured natural width, distribute leftover proportionally, and push final
  // rounding remainder onto the last column so totals match exactly.
  const resizerTotal = (columns.length - 1) * RESIZER_PX;
  const available = Math.max(0, containerPx - resizerTotal);
  const minColPx = columns.map((c) => naturalMinPxById[c.id] ?? DEFAULT_MIN_COL_PX);
  const colPx = computeColPx(columns.map((c) => c.width), available, minColPx);

  const parts: string[] = [];
  for (let i = 0; i < columns.length; i++) {
    if (i > 0) parts.push(`${RESIZER_PX}px`);
    parts.push(`${colPx[i]}px`);
  }
  const template = parts.join(" ");

  const startResize = (leftIdx: number) => (e: React.PointerEvent) => {
    e.preventDefault();
    const leftId = columns[leftIdx].id;
    const rightId = columns[leftIdx + 1].id;
    const measuredLeftMin = columnRefs.current.get(leftId);
    const measuredRightMin = columnRefs.current.get(rightId);
    const leftMin = measuredLeftMin ? measureColumnMinPx(measuredLeftMin) : minColPx[leftIdx];
    const rightMin = measuredRightMin ? measureColumnMinPx(measuredRightMin) : minColPx[leftIdx + 1];
    setNaturalMin(leftId, leftMin);
    setNaturalMin(rightId, rightMin);

    const leftPx0 = Math.max(colPx[leftIdx], leftMin);
    const rightPx0 = Math.max(colPx[leftIdx + 1], rightMin);
    const totalPx = leftPx0 + rightPx0;
    const totalWeight = columns[leftIdx].width + columns[leftIdx + 1].width;
    const startX = e.clientX;
    const target = e.currentTarget as HTMLElement;
    target.setPointerCapture(e.pointerId);
    const onMove = (ev: PointerEvent) => {
      const dx = ev.clientX - startX;
      const leftPx = clamp(leftPx0 + dx, leftMin, totalPx - rightMin);
      const rightPx = totalPx - leftPx;
      const leftW = totalWeight * (leftPx / totalPx);
      const rightW = totalWeight * (rightPx / totalPx);
      updateColumn(columns[leftIdx].id, { width: leftW });
      updateColumn(columns[leftIdx + 1].id, { width: rightW });
    };
    const onUp = () => {
      target.releasePointerCapture(e.pointerId);
      window.removeEventListener("pointermove", onMove);
      window.removeEventListener("pointerup", onUp);
    };
    window.addEventListener("pointermove", onMove);
    window.addEventListener("pointerup", onUp);
  };

  const nodes: React.ReactNode[] = [];
  for (let i = 0; i < columns.length; i++) {
    if (i > 0) {
      nodes.push(
        <div
          key={`resizer-${columns[i - 1].id}-${columns[i].id}`}
          className="col-resizer"
          onPointerDown={startResize(i - 1)}
          title="Drag to resize"
        />
      );
    }
    const c = columns[i];
    nodes.push(
      <div
        key={c.id}
        ref={(el) => {
          if (el) columnRefs.current.set(c.id, el);
          else columnRefs.current.delete(c.id);
        }}
        className="column"
      >
        <ColumnBody column={c} />
      </div>
    );
  }

  return (
    <div ref={wsRef} className="workspace" style={{ gridTemplateColumns: template }}>
      {nodes}
    </div>
  );
}

// Distribute `available` pixels across columns by weight, clamped at each
// column's natural minimum. If the minimums exceed the container, return the
// minimums and let the workspace's horizontal overflow handle it.
function computeColPx(weights: number[], available: number, minWidths: number[]): number[] {
  const n = weights.length;
  if (n === 0) return [];
  const mins = weights.map((_, i) => Math.max(DEFAULT_MIN_COL_PX, minWidths[i] ?? DEFAULT_MIN_COL_PX));
  const minTotal = mins.reduce((a, b) => a + b, 0);
  if (available <= minTotal) return mins;
  // First pass: ideal pixels by weight.
  const sumW = weights.reduce((a, b) => a + Math.max(0.001, b), 0);
  let raw = weights.map((w) => (Math.max(0.001, w) / sumW) * available);
  // Clamp to natural minimums, redistribute the deficit from the rest.
  let pinned = new Array<boolean>(n).fill(false);
  let changed = true;
  while (changed) {
    changed = false;
    let pinnedTotal = 0;
    let freeWeight = 0;
    for (let i = 0; i < n; i++) {
      if (pinned[i]) {
        pinnedTotal += mins[i];
      } else {
        freeWeight += Math.max(0.001, weights[i]);
      }
    }
    const freeAvail = Math.max(0, available - pinnedTotal);
    for (let i = 0; i < n; i++) {
      if (pinned[i]) continue;
      raw[i] = (Math.max(0.001, weights[i]) / Math.max(0.001, freeWeight)) * freeAvail;
      if (raw[i] < mins[i]) {
        pinned[i] = true;
        raw[i] = mins[i];
        changed = true;
      }
    }
  }
  // Round to integers and absorb remainder into the last unpinned column.
  const out = raw.map((p) => Math.floor(p));
  const used = out.reduce((a, b) => a + b, 0);
  const remainder = available - used;
  if (remainder !== 0) {
    let absorb = n - 1;
    for (let i = n - 1; i >= 0; i--) {
      if (!pinned[i]) { absorb = i; break; }
    }
    out[absorb] += remainder;
  }
  return out;
}

function measureColumnMinPx(el: HTMLElement): number {
  const clone = el.cloneNode(true) as HTMLElement;
  clone.setAttribute("aria-hidden", "true");
  clone.style.position = "absolute";
  clone.style.visibility = "hidden";
  clone.style.pointerEvents = "none";
  clone.style.left = "-10000px";
  clone.style.top = "0";
  clone.style.width = "min-content";
  clone.style.height = "auto";
  clone.style.maxWidth = "none";
  clone.style.overflow = "visible";

  document.body.appendChild(clone);
  const width = Math.ceil(clone.getBoundingClientRect().width);
  clone.remove();

  return Math.max(DEFAULT_MIN_COL_PX, width || DEFAULT_MIN_COL_PX);
}

function clamp(value: number, min: number, max: number): number {
  if (max < min) return min;
  return Math.max(min, Math.min(max, value));
}
