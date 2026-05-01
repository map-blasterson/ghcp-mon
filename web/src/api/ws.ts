import type { WsEntity, WsEnvelope, WsKind } from "./types";
import { WS_URL } from "./client";

type Listener = (env: WsEnvelope) => void;

class WsBus {
  private ws: WebSocket | null = null;
  private listeners = new Set<Listener>();
  private reconnectAttempt = 0;
  private reconnectTimer: number | null = null;
  private connected = false;
  private statusListeners = new Set<(connected: boolean) => void>();

  start(): void {
    if (this.ws) return;
    this.connect();
  }

  private connect(): void {
    try {
      this.ws = new WebSocket(WS_URL);
    } catch {
      this.scheduleReconnect();
      return;
    }
    this.ws.onopen = () => {
      this.reconnectAttempt = 0;
      this.connected = true;
      this.statusListeners.forEach((l) => l(true));
    };
    this.ws.onclose = () => {
      this.connected = false;
      this.statusListeners.forEach((l) => l(false));
      this.ws = null;
      this.scheduleReconnect();
    };
    this.ws.onerror = () => {
      this.ws?.close();
    };
    this.ws.onmessage = (ev) => {
      try {
        const env = JSON.parse(ev.data) as WsEnvelope;
        this.listeners.forEach((l) => l(env));
      } catch {
        // ignore malformed
      }
    };
  }

  private scheduleReconnect(): void {
    if (this.reconnectTimer != null) return;
    const delay = Math.min(30_000, 500 * 2 ** this.reconnectAttempt);
    this.reconnectAttempt += 1;
    this.reconnectTimer = window.setTimeout(() => {
      this.reconnectTimer = null;
      this.connect();
    }, delay);
  }

  on(l: Listener): () => void {
    this.listeners.add(l);
    return () => this.listeners.delete(l);
  }

  onStatus(l: (connected: boolean) => void): () => void {
    this.statusListeners.add(l);
    l(this.connected);
    return () => this.statusListeners.delete(l);
  }

  isConnected(): boolean {
    return this.connected;
  }
}

export const wsBus = new WsBus();

export type { WsEntity, WsEnvelope, WsKind };
