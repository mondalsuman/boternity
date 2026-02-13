/**
 * TypeScript types matching the Rust AgentEvent enum.
 *
 * Tagged union via `type` field, matching the serde convention:
 * #[serde(tag = "type", rename_all = "snake_case")]
 *
 * Used by the WebSocket hook and agent store to process real-time
 * agent hierarchy events from the backend.
 */

// -- Agent events (tagged union matching Rust AgentEvent) --

export type AgentEvent =
  | {
      type: "agent_spawned";
      agent_id: string;
      parent_id: string | null;
      task_description: string;
      depth: number;
      index: number;
      total: number;
    }
  | { type: "agent_text_delta"; agent_id: string; text: string }
  | {
      type: "agent_completed";
      agent_id: string;
      result_summary: string;
      tokens_used: number;
      duration_ms: number;
    }
  | {
      type: "agent_failed";
      agent_id: string;
      error: string;
      will_retry: boolean;
    }
  | { type: "agent_cancelled"; agent_id: string; reason: string }
  | {
      type: "budget_update";
      request_id: string;
      tokens_used: number;
      budget_total: number;
      percentage: number;
    }
  | {
      type: "budget_warning";
      request_id: string;
      tokens_used: number;
      budget_total: number;
    }
  | {
      type: "budget_exhausted";
      request_id: string;
      tokens_used: number;
      budget_total: number;
      completed_agents: string[];
      incomplete_agents: string[];
    }
  | {
      type: "depth_limit_reached";
      agent_id: string;
      attempted_depth: number;
      max_depth: number;
    }
  | {
      type: "cycle_detected";
      agent_id: string;
      cycle_description: string;
    }
  | { type: "synthesis_started"; request_id: string }
  | { type: "memory_created"; agent_id: string; fact: string }
  | {
      type: "provider_failover";
      from_provider: string;
      to_provider: string;
      reason: string;
    };

// -- Agent node state for the tree --

export type AgentStatus =
  | "pending"
  | "running"
  | "completed"
  | "failed"
  | "cancelled";

export interface AgentNode {
  agentId: string;
  parentId: string | null;
  task: string;
  depth: number;
  index: number;
  total: number;
  status: AgentStatus;
  streamedText: string;
  tokensUsed: number;
  durationMs: number;
}

// -- WebSocket connection status --

export type WsConnectionStatus = "connected" | "reconnecting" | "disconnected";
