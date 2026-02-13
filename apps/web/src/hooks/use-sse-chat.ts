/**
 * SSE streaming chat hook using fetch + ReadableStream.
 *
 * Uses POST with JSON body (NOT EventSource which only supports GET).
 * Parses SSE events: session, text_delta, usage, done, error.
 * Also handles agent hierarchy events: agent_spawned, agent_text_delta,
 * agent_completed, agent_failed, agent_cancelled, budget_update,
 * budget_warning, budget_exhausted, synthesis_started.
 *
 * Agent events are forwarded to the Zustand agent store.
 * AbortController for stop generation and cleanup on unmount.
 */

import { useState, useCallback, useRef, useEffect } from "react";
import { useApiKeyStore } from "@/stores/api-key-store";
import { useAgentStore } from "@/stores/agent-store";
import type { AgentEvent } from "@/types/agent";

export interface StreamUsage {
  input_tokens: number;
  output_tokens: number;
}

export function useSSEChat() {
  const [isStreaming, setIsStreaming] = useState(false);
  const [streamedContent, setStreamedContent] = useState("");
  const [activeSessionId, setActiveSessionId] = useState<string | null>(null);
  const [usage, setUsage] = useState<StreamUsage | null>(null);
  const [error, setError] = useState<string | null>(null);
  const abortRef = useRef<AbortController | null>(null);

  const sendMessage = useCallback(
    async (
      botId: string,
      message: string,
      sessionId?: string,
    ): Promise<string | null> => {
      setIsStreaming(true);
      setStreamedContent("");
      setError(null);
      setUsage(null);
      // Reset agent store for new request
      useAgentStore.getState().reset();
      abortRef.current = new AbortController();

      // Track session ID locally so the caller gets the resolved value
      // (avoids stale closure issues with state)
      let resolvedSessionId: string | null = sessionId ?? null;

      try {
        const apiKey = useApiKeyStore.getState().apiKey;
        const res = await fetch(`/api/v1/bots/${botId}/chat/stream`, {
          method: "POST",
          headers: {
            "Content-Type": "application/json",
            ...(apiKey ? { "X-API-Key": apiKey } : {}),
          },
          body: JSON.stringify({ session_id: sessionId, message }),
          signal: abortRef.current.signal,
        });

        if (!res.ok) {
          throw new Error(`Server returned ${res.status}: ${res.statusText}`);
        }

        const reader = res.body!.getReader();
        const decoder = new TextDecoder();
        let buffer = "";
        let currentEventType = "";

        while (true) {
          const { done, value } = await reader.read();
          if (done) break;
          buffer += decoder.decode(value, { stream: true });

          const lines = buffer.split("\n");
          buffer = lines.pop()!; // Keep incomplete line in buffer

          for (const line of lines) {
            if (line.startsWith("event: ")) {
              currentEventType = line.slice(7).trim();
              continue;
            }
            if (line.startsWith("data: ")) {
              const jsonStr = line.slice(6);
              try {
                const event = JSON.parse(jsonStr);

                switch (currentEventType) {
                  case "session":
                    resolvedSessionId = event.session_id;
                    setActiveSessionId(event.session_id);
                    break;
                  case "text_delta":
                    // CRITICAL: functional updater to avoid stale closure
                    setStreamedContent((prev) => prev + event.text);
                    break;
                  case "usage":
                    setUsage({
                      input_tokens: event.input_tokens,
                      output_tokens: event.output_tokens,
                    });
                    break;
                  case "done":
                    // Stream complete
                    break;
                  case "error":
                    setError(event.message || "Unknown streaming error");
                    break;

                  // Agent hierarchy events -- forward to agent store
                  case "agent_spawned":
                  case "agent_text_delta":
                  case "agent_completed":
                  case "agent_failed":
                  case "agent_cancelled":
                  case "budget_update":
                  case "budget_warning":
                  case "budget_exhausted":
                    useAgentStore
                      .getState()
                      .handleEvent(event as AgentEvent);
                    break;

                  case "synthesis_started":
                    // Reset streamed content for the synthesis phase
                    setStreamedContent("");
                    break;
                }
              } catch {
                // Ignore malformed JSON lines
              }
            }
          }
        }
      } catch (err: unknown) {
        // Ignore AbortError (user cancelled), rethrow others
        if (err instanceof Error && err.name === "AbortError") {
          // Normal cancellation
        } else {
          const message =
            err instanceof Error ? err.message : "Streaming failed";
          setError(message);
        }
      } finally {
        setIsStreaming(false);
      }

      return resolvedSessionId;
    },
    [],
  );

  const stopGeneration = useCallback(() => {
    abortRef.current?.abort();
    setIsStreaming(false);
  }, []);

  // Cleanup on unmount
  useEffect(() => {
    return () => {
      abortRef.current?.abort();
    };
  }, []);

  const clearStreamedContent = useCallback(() => {
    setStreamedContent("");
  }, []);

  return {
    sendMessage,
    stopGeneration,
    clearStreamedContent,
    streamedContent,
    isStreaming,
    activeSessionId,
    usage,
    error,
  };
}
