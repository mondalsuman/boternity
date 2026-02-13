/**
 * Custom hook connecting the WebSocket to the agent store.
 *
 * Subscribes to WebSocket events and forwards them to the Zustand agent store.
 * Returns agent tree state, connection info, and command helpers for
 * cancellation and budget control.
 */

import { useEffect } from "react";
import { useAgentWebSocket } from "@/hooks/use-websocket";
import { useAgentStore } from "@/stores/agent-store";
import type { AgentEvent } from "@/types/agent";

export function useAgentTree() {
  const ws = useAgentWebSocket(
    `${window.location.protocol === "https:" ? "wss:" : "ws:"}//${window.location.host}/ws/events`,
  );
  const handleEvent = useAgentStore((s) => s.handleEvent);
  const agents = useAgentStore((s) => s.agents);
  const tokensUsed = useAgentStore((s) => s.tokensUsed);
  const budgetTotal = useAgentStore((s) => s.budgetTotal);
  const budgetPercentage = useAgentStore((s) => s.budgetPercentage);
  const budgetWarning = useAgentStore((s) => s.budgetWarning);
  const budgetExhausted = useAgentStore((s) => s.budgetExhausted);
  const reset = useAgentStore((s) => s.reset);

  useEffect(() => {
    return ws.onEvent((event) => {
      handleEvent(event as AgentEvent);
    });
  }, [ws, handleEvent]);

  return {
    agents,
    status: ws.status,
    sendCommand: ws.sendCommand,
    budgetInfo: {
      tokensUsed,
      budgetTotal,
      percentage: budgetPercentage,
      warning: budgetWarning,
      exhausted: budgetExhausted,
    },
    cancelAgent: (agentId: string) => {
      ws.sendCommand({ type: "cancel_agent", agent_id: agentId });
    },
    budgetContinue: (requestId: string) => {
      ws.sendCommand({ type: "budget_continue", request_id: requestId });
    },
    budgetStop: (requestId: string) => {
      ws.sendCommand({ type: "budget_stop", request_id: requestId });
    },
    reset,
  };
}
