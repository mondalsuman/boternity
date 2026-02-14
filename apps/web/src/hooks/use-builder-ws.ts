/**
 * WebSocket hook for the Forge builder chat.
 *
 * Connects to /ws/builder/{sessionId} for real-time builder conversation.
 * Follows the Phase 5 WebSocket pattern: native WebSocket API with
 * exponential backoff (1s-30s, 30% jitter, max 10 attempts).
 *
 * Supports both bot and skill creation modes via distinct start messages.
 * Dispatches incoming turns to the forge store and shows error toasts.
 */

import { useState, useRef, useCallback, useEffect } from "react";
import { toast } from "sonner";
import { useForgeStore } from "@/stores/forge-store";
import type {
  BuilderTurn,
  BuilderAnswer,
  BuilderConfig,
  BuilderPreview,
  AssemblyResult,
} from "@/lib/api/builder";
import type { SkillCreatedResult } from "@/stores/forge-store";

// ---------------------------------------------------------------------------
// Types matching the Rust WsBuilderMessage / WsBuilderResponse
// ---------------------------------------------------------------------------

/** Client -> Server messages. Tagged union on `type` field. */
type WsBuilderMessage =
  | { type: "start_bot"; description: string }
  | { type: "start_skill"; description: string }
  | { type: "answer"; answer: BuilderAnswer }
  | { type: "assemble_bot"; config: BuilderConfig }
  | { type: "create_skill"; skill_request: SkillRequestDto }
  | { type: "resume" }
  | { type: "ping" };

/** Skill request DTO for the CreateSkill message. */
export interface SkillRequestDto {
  name: string;
  description: string;
  skill_type?: string;
  capabilities?: string[];
}

/** Server -> Client messages. Tagged union on `type` field. */
type WsBuilderResponse =
  | { type: "turn"; turn: BuilderTurn }
  | { type: "bot_assembled"; result: AssemblyResult }
  | { type: "skill_created"; result: SkillCreatedResult }
  | { type: "error"; message: string }
  | { type: "pong" };

// ---------------------------------------------------------------------------
// Reconnection helpers
// ---------------------------------------------------------------------------

/**
 * Calculate reconnection delay with exponential backoff and jitter.
 * Base: 1000ms, doubles each attempt, capped at 30000ms.
 * Jitter: +/- 30% to prevent thundering herd.
 */
function getReconnectDelay(attempt: number): number {
  const base = Math.min(1000 * Math.pow(2, attempt), 30000);
  const jitter = base * 0.3 * (Math.random() * 2 - 1);
  return Math.max(0, base + jitter);
}

const MAX_ATTEMPTS = 10;

// ---------------------------------------------------------------------------
// Hook
// ---------------------------------------------------------------------------

export type BuilderWsStatus = "connected" | "reconnecting" | "disconnected";

export function useBuilderWs(sessionId: string | null) {
  const [status, setStatus] = useState<BuilderWsStatus>("disconnected");
  const [error, setError] = useState<string | null>(null);

  const wsRef = useRef<WebSocket | null>(null);
  const attemptRef = useRef(0);
  const reconnectTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const sessionIdRef = useRef(sessionId);
  sessionIdRef.current = sessionId;

  // Store actions (stable references from zustand)
  const addForgeMessage = useForgeStore((s) => s.addForgeMessage);
  const setAssembled = useForgeStore((s) => s.setAssembled);
  const setSkillCreated = useForgeStore((s) => s.setSkillCreated);
  const setPreview = useForgeStore((s) => s.setPreview);

  // -----------------------------------------------------------------------
  // Message handler
  // -----------------------------------------------------------------------

  const handleMessage = useCallback(
    (data: WsBuilderResponse) => {
      switch (data.type) {
        case "turn": {
          addForgeMessage(data.turn);
          // If the turn contains a preview update, apply it
          if (
            data.turn.action === "show_preview" &&
            "preview" in data.turn
          ) {
            setPreview(data.turn.preview as BuilderPreview);
          }
          break;
        }
        case "bot_assembled": {
          setAssembled(data.result);
          toast.success(
            `Bot "${data.result.bot_name}" created successfully!`,
          );
          break;
        }
        case "skill_created": {
          setSkillCreated(data.result);
          toast.success(
            `Skill "${data.result.name}" created successfully!`,
          );
          break;
        }
        case "error": {
          setError(data.message);
          toast.error(data.message);
          break;
        }
        case "pong":
          // Heartbeat response, no action needed
          break;
      }
    },
    [addForgeMessage, setAssembled, setSkillCreated, setPreview],
  );

  // -----------------------------------------------------------------------
  // Connection management
  // -----------------------------------------------------------------------

  const connect = useCallback(() => {
    const sid = sessionIdRef.current;
    if (!sid) return;

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
      const protocol = window.location.protocol === "https:" ? "wss:" : "ws:";
      const url = `${protocol}//${window.location.host}/ws/builder/${sid}`;
      const ws = new WebSocket(url);
      wsRef.current = ws;

      ws.onopen = () => {
        attemptRef.current = 0;
        setStatus("connected");
        setError(null);
      };

      ws.onmessage = (event: MessageEvent) => {
        try {
          const data = JSON.parse(event.data) as WsBuilderResponse;
          handleMessage(data);
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
  }, [handleMessage]);

  // -----------------------------------------------------------------------
  // Send helpers
  // -----------------------------------------------------------------------

  const send = useCallback((msg: WsBuilderMessage) => {
    if (wsRef.current?.readyState === WebSocket.OPEN) {
      wsRef.current.send(JSON.stringify(msg));
    }
  }, []);

  const sendStartBot = useCallback(
    (description: string) => {
      send({ type: "start_bot", description });
    },
    [send],
  );

  const sendStartSkill = useCallback(
    (description: string) => {
      send({ type: "start_skill", description });
    },
    [send],
  );

  const sendAnswer = useCallback(
    (answer: BuilderAnswer) => {
      send({ type: "answer", answer });
    },
    [send],
  );

  const sendAssembleBot = useCallback(
    (config: BuilderConfig) => {
      send({ type: "assemble_bot", config });
    },
    [send],
  );

  const sendCreateSkill = useCallback(
    (skillRequest: SkillRequestDto) => {
      send({ type: "create_skill", skill_request: skillRequest });
    },
    [send],
  );

  const sendResume = useCallback(() => {
    send({ type: "resume" });
  }, [send]);

  // -----------------------------------------------------------------------
  // Lifecycle
  // -----------------------------------------------------------------------

  useEffect(() => {
    if (!sessionId) return;

    connect();

    return () => {
      if (reconnectTimerRef.current) {
        clearTimeout(reconnectTimerRef.current);
        reconnectTimerRef.current = null;
      }
      if (wsRef.current) {
        wsRef.current.onopen = null;
        wsRef.current.onclose = null;
        wsRef.current.onmessage = null;
        wsRef.current.onerror = null;
        wsRef.current.close();
        wsRef.current = null;
      }
    };
  }, [sessionId, connect]);

  return {
    status,
    error,
    isConnected: status === "connected",
    sendStartBot,
    sendStartSkill,
    sendAnswer,
    sendAssembleBot,
    sendCreateSkill,
    sendResume,
  };
}
