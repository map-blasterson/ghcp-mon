import { useEffect, useRef, useState } from "react";
import { wsBus } from "../api/ws";
import type { WsEntity, WsEnvelope, WsKind } from "../api/types";

const RING_MAX = 500;

interface Ring {
  envelopes: WsEnvelope[];
  count: number;
}

const rings = new Map<string, Ring>();
const subscribers = new Map<string, Set<() => void>>();
let started = false;

function key(kind: WsKind, entity: WsEntity): string {
  return `${kind}/${entity}`;
}

function ensureStarted(): void {
  if (started) return;
  started = true;
  wsBus.start();
  wsBus.on((env) => {
    const k = key(env.kind, env.entity);
    const ring = rings.get(k) ?? { envelopes: [], count: 0 };
    ring.envelopes.unshift(env);
    if (ring.envelopes.length > RING_MAX) ring.envelopes.length = RING_MAX;
    ring.count += 1;
    rings.set(k, ring);
    subscribers.get(k)?.forEach((fn) => fn());
    subscribers.get("*")?.forEach((fn) => fn());
  });
}

export function useLiveFeed(
  filters: Array<{ kind: WsKind; entity: WsEntity }>
): { envelopes: WsEnvelope[]; tick: number } {
  ensureStarted();
  const [tick, setTick] = useState(0);
  const filterRef = useRef(filters);
  filterRef.current = filters;
  useEffect(() => {
    const cb = () => setTick((t) => t + 1);
    const keys = filters.map((f) => key(f.kind, f.entity));
    keys.forEach((k) => {
      let s = subscribers.get(k);
      if (!s) {
        s = new Set();
        subscribers.set(k, s);
      }
      s.add(cb);
    });
    return () => {
      keys.forEach((k) => subscribers.get(k)?.delete(cb));
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [filters.map((f) => key(f.kind, f.entity)).join("|")]);

  const envelopes = filters.flatMap((f) => rings.get(key(f.kind, f.entity))?.envelopes ?? []);
  return { envelopes, tick };
}

export function useWsStatus(): boolean {
  ensureStarted();
  const [c, setC] = useState(wsBus.isConnected());
  useEffect(() => wsBus.onStatus(setC), []);
  return c;
}
