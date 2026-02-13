/**
 * Process-manager style agent tree panel.
 *
 * Per user decision: "Web: agent tree panel (like a process manager) with
 * per-agent stop buttons, always togglable via a button."
 *
 * Shows a tree view of all agents with status badges, token counts,
 * durations, and stop buttons for running agents. Uses depth-based
 * indentation (pl-4 per level).
 */

import { memo } from "react";
import {
  Loader2,
  Check,
  X,
  Ban,
  Square,
  Network,
} from "lucide-react";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { ScrollArea } from "@/components/ui/scroll-area";
import { cn } from "@/lib/utils";
import { useAgentStore } from "@/stores/agent-store";
import type { AgentNode, AgentStatus } from "@/types/agent";

interface AgentTreePanelProps {
  onCancelAgent: (agentId: string) => void;
}

/** Status badge variant */
function statusBadgeVariant(
  status: AgentStatus,
): "default" | "secondary" | "destructive" | "outline" {
  switch (status) {
    case "running":
      return "default";
    case "completed":
      return "secondary";
    case "failed":
      return "destructive";
    case "cancelled":
      return "outline";
    case "pending":
      return "outline";
  }
}

/** Small status icon for tree nodes */
function TreeStatusIcon({ status }: { status: AgentStatus }) {
  switch (status) {
    case "running":
      return <Loader2 className="size-3 animate-spin" />;
    case "completed":
      return <Check className="size-3" />;
    case "failed":
      return <X className="size-3" />;
    case "cancelled":
      return <Ban className="size-3" />;
    case "pending":
      return <div className="size-3 rounded-full border border-current" />;
  }
}

/** Format token count */
function formatTokens(tokens: number): string {
  if (tokens >= 1000) return `${(tokens / 1000).toFixed(1)}k`;
  return String(tokens);
}

/** Format duration */
function formatDuration(ms: number): string {
  if (ms < 1000) return `${ms}ms`;
  return `${(ms / 1000).toFixed(1)}s`;
}

/** Recursive tree node component */
const TreeNode = memo(function TreeNode({
  agent,
  onCancel,
}: {
  agent: AgentNode;
  onCancel: (agentId: string) => void;
}) {
  const getAgentChildren = useAgentStore((s) => s.getAgentChildren);
  const children = getAgentChildren(agent.agentId);

  // Truncate task description for tree view
  const taskLabel =
    agent.task.length > 50 ? agent.task.slice(0, 47) + "..." : agent.task;

  return (
    <div>
      <div
        className="flex items-center gap-2 py-1.5 px-2 rounded hover:bg-muted/50 group"
        style={{ paddingLeft: `${(agent.depth + 1) * 16}px` }}
      >
        {/* Status icon */}
        <TreeStatusIcon status={agent.status} />

        {/* Agent info */}
        <div className="flex-1 min-w-0">
          <div className="flex items-center gap-1.5">
            <span className="text-xs font-medium truncate">{taskLabel}</span>
            <Badge
              variant={statusBadgeVariant(agent.status)}
              className="text-[10px] px-1.5 py-0"
            >
              {agent.status}
            </Badge>
          </div>

          {/* Metadata row */}
          <div className="flex items-center gap-2 text-[10px] text-muted-foreground">
            {agent.tokensUsed > 0 && (
              <span>{formatTokens(agent.tokensUsed)} tok</span>
            )}
            {agent.durationMs > 0 && (
              <span>{formatDuration(agent.durationMs)}</span>
            )}
          </div>
        </div>

        {/* Stop button for running agents */}
        {agent.status === "running" && (
          <Button
            variant="ghost"
            size="icon-xs"
            className="opacity-0 group-hover:opacity-100 text-destructive hover:text-destructive hover:bg-destructive/10 shrink-0"
            onClick={() => onCancel(agent.agentId)}
            title="Stop agent"
          >
            <Square className="size-3" />
          </Button>
        )}
      </div>

      {/* Recursive children */}
      {children.map((child: AgentNode) => (
        <TreeNode key={child.agentId} agent={child} onCancel={onCancel} />
      ))}
    </div>
  );
});

export const AgentTreePanel = memo(function AgentTreePanel({
  onCancelAgent,
}: AgentTreePanelProps) {
  const agents = useAgentStore((s) => s.agents);
  const getAgentTree = useAgentStore((s) => s.getAgentTree);
  const rootAgents = getAgentTree();

  // Count running agents
  let runningCount = 0;
  for (const agent of agents.values()) {
    if (agent.status === "running") runningCount++;
  }

  if (agents.size === 0) {
    return (
      <div className="flex flex-col items-center justify-center py-8 text-center">
        <Network className="size-8 text-muted-foreground/30 mb-2" />
        <p className="text-xs text-muted-foreground">
          No agents active
        </p>
      </div>
    );
  }

  return (
    <div className="flex flex-col h-full">
      {/* Header */}
      <div className="flex items-center justify-between px-3 py-2 border-b">
        <div className="flex items-center gap-2">
          <Network className="size-4 text-muted-foreground" />
          <span className="text-xs font-medium">Agent Tree</span>
        </div>
        <div className="flex items-center gap-2 text-[10px] text-muted-foreground">
          <span>{agents.size} total</span>
          {runningCount > 0 && (
            <Badge variant="default" className={cn("text-[10px] px-1.5 py-0")}>
              {runningCount} running
            </Badge>
          )}
        </div>
      </div>

      {/* Tree body */}
      <ScrollArea className="flex-1">
        <div className="py-1">
          {rootAgents.map((agent: AgentNode) => (
            <TreeNode
              key={agent.agentId}
              agent={agent}
              onCancel={onCancelAgent}
            />
          ))}
        </div>
      </ScrollArea>
    </div>
  );
});
