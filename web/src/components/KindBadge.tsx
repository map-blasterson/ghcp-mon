import type { KindClass } from "../api/types";

// Display label for a KindClass. We rename "execute_tool" → "tool" so
// the dashboard reads naturally; the wire/DB representation is unchanged.
export function kindLabel(k: KindClass): string {
  switch (k) {
    case "execute_tool":
      return "tool";
    case "external_tool":
      return "external";
    case "invoke_agent":
      return "agent";
    case "other":
      return "pending";
    default:
      return k;
  }
}

export function kindClass(k: KindClass): string {
  return `kind kind-${k.replace("_", "-")}`;
}

export function KindBadge({ k }: { k: KindClass }) {
  return <span className={kindClass(k)}>{kindLabel(k)}</span>;
}

// Deterministic 32-bit FNV-1a hash. Stable across reloads/sessions so
// the same string always maps to the same hue.
function hashStr(s: string): number {
  let h = 0x811c9dc5;
  for (let i = 0; i < s.length; i++) {
    h ^= s.charCodeAt(i);
    h = Math.imul(h, 0x01000193);
  }
  return h >>> 0;
}

// Hash an arbitrary label to a stable HSL color. Uses high saturation +
// medium lightness so the chip is readable on a dark theme; avoids the
// muddy yellow-green band by skewing hue selection.
export function hashColor(s: string): string {
  const hue = hashStr(s) % 360;
  return `hsl(${hue}, 65%, 68%)`;
}

export function HashTag({ label }: { label: string }) {
  const c = hashColor(label);
  return (
    <span className="kind" style={{ color: c, marginRight: 4 }}>
      {label}
    </span>
  );
}

// Animated rolling-dots indicator. Used as the visual content of a
// placeholder tag — three dots that pulse in a wave so the user can
// see the row is still waiting on real data.
export function RollingDots() {
  return (
    <span className="rolling-dots" aria-label="pending">
      <i />
      <i />
      <i />
    </span>
  );
}
