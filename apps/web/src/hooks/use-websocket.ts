/**
 * WebSocket hook with exponential backoff reconnection.
 *
 * Uses native WebSocket API (no npm dependency per research decision).
 * Exponential backoff: 1s base, doubling to max 30s, with 30% jitter.
 * Max 10 reconnection attempts before giving up.
 *
 * Returns connection status, sendCommand, and onEvent for event subscription.
 */

import { useState, useRef, useCallback, useEffect } from "react";
import type { WsConnectionStatus } from "@/types/agent";

type EventListener = (event: unknown) => void;

/**
 * Calculate reconnection delay with exponential backoff and jitter.
 * Base: 1000ms, doubles each attempt, capped at 30000ms.
 * Jitter: +/- 30% to prevent thundering herd.
 */
function getReconnectDelay(attempt: number): number {
  const base = Math.min(1000 * Math.pow(2, attempt), 30000);
  const jitter = base * 0.3 * (Math.random() * 2 - 1); // +/- 30%
  return Math.max(0, base + jitter);
}

const MAX_ATTEMPTS = 10;

export function useAgentWebSocket(url: string) {
  const [status, setStatus] = useState<WsConnectionStatus>("disconnected");
  const wsRef = useRef<WebSocket | null>(null);
  const attemptRef = useRef(0);
  const reconnectTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const listenersRef = useRef<Set<EventListener>>(new Set());
  const urlRef = useRef(url);
  urlRef.current = url;

  const connect = useCallback(() => {
    // Clean up existing connection
    if (wsRef.current) {
      wsRef.current.onopen = null;
      wsRef.current.onclose = null;
      wsRef.current.onmessage = null;
      wsRef.current.onerror = null;
      if (
        wsRef.current.readyState === WebSocket.OPEN ||
        wsRef.current.readyState === WebSocket.CONNECTING
      ) {
        wsRef.current.close();
      }
    }

    try {
      const ws = new WebSocket(urlRef.current);
      wsRef.current = ws;

      ws.onopen = () => {
        attemptRef.current = 0;
        setStatus("connected");
      };

      ws.onmessage = (messageEvent: MessageEvent) => {
        try {
          const data = JSON.parse(messageEvent.data);
          for (const listener of listenersRef.current) {
            listener(data);
          }
        } catch {
          // Ignore malformed JSON
        }
      };

      ws.onclose = () => {
        wsRef.current = null;

        if (attemptRef.current < MAX_ATTEMPTS) {
          setStatus("reconnecting");
          const delay = getReconnectDelay(attemptRef.current);
          attemptRef.current += 1;
          reconnectTimerRef.current = setTimeout(() => {
            connect();
          }, delay);
        } else {
          setStatus("disconnected");
        }
      };

      ws.onerror = () => {
        // Error triggers onclose, which handles reconnection
      };
    } catch {
      // Connection creation failed - trigger reconnection
      if (attemptRef.current < MAX_ATTEMPTS) {
        setStatus("reconnecting");
        const delay = getReconnectDelay(attemptRef.current);
        attemptRef.current += 1;
        reconnectTimerRef.current = setTimeout(() => {
          connect();
        }, delay);
      } else {
        setStatus("disconnected");
      }
    }
  }, []);

  const sendCommand = useCallback((cmd: object) => {
    if (wsRef.current?.readyState === WebSocket.OPEN) {
      wsRef.current.send(JSON.stringify(cmd));
    }
  }, []);

  const onEvent = useCallback((fn: EventListener): (() => void) => {
    listenersRef.current.add(fn);
    return () => {
      listenersRef.current.delete(fn);
    };
  }, []);

  // Connect on mount, clean up on unmount
  useEffect(() => {
    connect();

    return () => {
      // Clear reconnect timer
      if (reconnectTimerRef.current) {
        clearTimeout(reconnectTimerRef.current);
        reconnectTimerRef.current = null;
      }
      // Close WebSocket
      if (wsRef.current) {
        wsRef.current.onopen = null;
        wsRef.current.onclose = null;
        wsRef.current.onmessage = null;
        wsRef.current.onerror = null;
        wsRef.current.close();
        wsRef.current = null;
      }
    };
  }, [connect]);

  return { status, sendCommand, onEvent };
}
