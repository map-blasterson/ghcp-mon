import { useLayoutEffect, useRef, useState } from "react";
import { useWorkspace } from "../state/workspace";
import { ColumnBody } from "./Column";

const RESIZER_PX = 4;
const MIN_COL_PX = 120;

export function Workspace() {
  const columns = useWorkspace((s) => s.columns);
  const updateColumn = useWorkspace((s) => s.updateColumn);
  const wsRef = useRef<HTMLDivElement>(null);
  const [containerPx, setContainerPx] = useState(0);

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

  if (columns.length === 0) {
    return <div className="empty-state">no columns. add one from the top bar.</div>;
  }

  // Distribute the available pixel width by weight (the persisted "width"
  // value, treated as a relative weight). Min-clamp each column to
  // MIN_COL_PX, distribute leftover proportionally, push final rounding
  // remainder onto the last column so totals match exactly.
  const resizerTotal = (columns.length - 1) * RESIZER_PX;
  const available = Math.max(0, containerPx - resizerTotal);
  const colPx = computeColPx(columns.map((c) => c.width), available);

  const parts: string[] = [];
  for (let i = 0; i < columns.length; i++) {
    if (i > 0) parts.push(`${RESIZER_PX}px`);
    parts.push(`${colPx[i]}px`);
  }
  const template = parts.join(" ");

  const startResize = (leftIdx: number) => (e: React.PointerEvent) => {
    e.preventDefault();
    const leftPx0 = colPx[leftIdx];
    const rightPx0 = colPx[leftIdx + 1];
    const totalPx = leftPx0 + rightPx0;
    const totalWeight = columns[leftIdx].width + columns[leftIdx + 1].width;
    const startX = e.clientX;
    const target = e.currentTarget as HTMLElement;
    target.setPointerCapture(e.pointerId);
    const onMove = (ev: PointerEvent) => {
      const dx = ev.clientX - startX;
      const leftPx = Math.max(MIN_COL_PX, Math.min(totalPx - MIN_COL_PX, leftPx0 + dx));
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
      <div key={c.id} className="column">
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

// Distribute `available` pixels across columns by weight, clamped at
// MIN_COL_PX. Returns integer pixel widths whose sum equals `available`
// (modulo the case where all minimums exceed available).
function computeColPx(weights: number[], available: number): number[] {
  const n = weights.length;
  if (n === 0) return [];
  if (available <= 0) return weights.map(() => MIN_COL_PX);
  // First pass: ideal pixels by weight.
  const sumW = weights.reduce((a, b) => a + Math.max(0.001, b), 0);
  let raw = weights.map((w) => (Math.max(0.001, w) / sumW) * available);
  // Clamp to MIN_COL_PX, redistribute the deficit from the rest.
  let pinned = new Array<boolean>(n).fill(false);
  let changed = true;
  while (changed) {
    changed = false;
    let pinnedTotal = 0;
    let freeWeight = 0;
    for (let i = 0; i < n; i++) {
      if (pinned[i]) {
        pinnedTotal += MIN_COL_PX;
      } else {
        freeWeight += Math.max(0.001, weights[i]);
      }
    }
    const freeAvail = Math.max(0, available - pinnedTotal);
    for (let i = 0; i < n; i++) {
      if (pinned[i]) continue;
      raw[i] = (Math.max(0.001, weights[i]) / Math.max(0.001, freeWeight)) * freeAvail;
      if (raw[i] < MIN_COL_PX) {
        pinned[i] = true;
        raw[i] = MIN_COL_PX;
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

