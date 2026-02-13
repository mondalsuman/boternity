/**
 * Zustand store for agent tree state and budget tracking.
 *
 * Processes AgentEvent messages from the WebSocket connection to maintain
 * a live view of all active/completed agents in the hierarchy, plus
 * budget usage and warning state.
 *
 * Uses plain functional updates (no immer) since Map operations are
 * straightforward with new Map(prev) spread.
 */

import { create } from "zustand";
import type { AgentEvent, AgentNode } from "@/types/agent";

interface AgentStore {
  // Agent tree
  agents: Map<string, AgentNode>;
  rootAgentIds: string[];

  // Budget
  tokensUsed: number;
  budgetTotal: number;
  budgetPercentage: number;
  budgetWarning: boolean;
  budgetExhausted: boolean;

  // Actions
  handleEvent: (event: AgentEvent) => void;
  reset: () => void;
  getAgentChildren: (agentId: string) => AgentNode[];
  getAgentTree: () => AgentNode[];
}

const initialState = {
  agents: new Map<string, AgentNode>(),
  rootAgentIds: [] as string[],
  tokensUsed: 0,
  budgetTotal: 0,
  budgetPercentage: 0,
  budgetWarning: false,
  budgetExhausted: false,
};

export const useAgentStore = create<AgentStore>()((set, get) => ({
  ...initialState,

  handleEvent: (event: AgentEvent) => {
    switch (event.type) {
      case "agent_spawned": {
        set((state) => {
          const agents = new Map(state.agents);
          agents.set(event.agent_id, {
            agentId: event.agent_id,
            parentId: event.parent_id,
            task: event.task_description,
            depth: event.depth,
            index: event.index,
            total: event.total,
            status: "running",
            streamedText: "",
            tokensUsed: 0,
            durationMs: 0,
          });

          const rootAgentIds =
            event.parent_id === null
              ? [...state.rootAgentIds, event.agent_id]
              : state.rootAgentIds;

          return { agents, rootAgentIds };
        });
        break;
      }

      case "agent_text_delta": {
        set((state) => {
          const agents = new Map(state.agents);
          const agent = agents.get(event.agent_id);
          if (agent) {
            agents.set(event.agent_id, {
              ...agent,
              streamedText: agent.streamedText + event.text,
            });
          }
          return { agents };
        });
        break;
      }

      case "agent_completed": {
        set((state) => {
          const agents = new Map(state.agents);
          const agent = agents.get(event.agent_id);
          if (agent) {
            agents.set(event.agent_id, {
              ...agent,
              status: "completed",
              tokensUsed: event.tokens_used,
              durationMs: event.duration_ms,
            });
          }
          return { agents };
        });
        break;
      }

      case "agent_failed": {
        set((state) => {
          const agents = new Map(state.agents);
          const agent = agents.get(event.agent_id);
          if (agent) {
            agents.set(event.agent_id, {
              ...agent,
              status: "failed",
            });
          }
          return { agents };
        });
        break;
      }

      case "agent_cancelled": {
        set((state) => {
          const agents = new Map(state.agents);
          const agent = agents.get(event.agent_id);
          if (agent) {
            agents.set(event.agent_id, {
              ...agent,
              status: "cancelled",
            });
          }
          return { agents };
        });
        break;
      }

      case "budget_update": {
        set({
          tokensUsed: event.tokens_used,
          budgetTotal: event.budget_total,
          budgetPercentage: event.percentage,
        });
        break;
      }

      case "budget_warning": {
        set({
          budgetWarning: true,
          tokensUsed: event.tokens_used,
          budgetTotal: event.budget_total,
        });
        break;
      }

      case "budget_exhausted": {
        set({
          budgetExhausted: true,
          tokensUsed: event.tokens_used,
          budgetTotal: event.budget_total,
        });
        break;
      }

      // These events are handled by UI components directly (toasts, inline)
      // or don't need store state:
      // - depth_limit_reached
      // - cycle_detected
      // - synthesis_started
      // - memory_created
      // - provider_failover
      default:
        break;
    }
  },

  reset: () => {
    set(initialState);
  },

  getAgentChildren: (agentId: string): AgentNode[] => {
    const { agents } = get();
    const children: AgentNode[] = [];
    for (const agent of agents.values()) {
      if (agent.parentId === agentId) {
        children.push(agent);
      }
    }
    // Sort by index for consistent ordering
    return children.sort((a, b) => a.index - b.index);
  },

  getAgentTree: (): AgentNode[] => {
    const { rootAgentIds, agents } = get();
    return rootAgentIds
      .map((id) => agents.get(id))
      .filter((node): node is AgentNode => node !== undefined);
  },
}));
