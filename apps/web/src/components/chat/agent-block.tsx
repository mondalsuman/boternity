/**
 * Collapsible sub-agent block rendered inline in chat.
 *
 * Per user decision: "Sub-agent activity shown inline in chat as collapsible
 * blocks (like Claude Code's tool use output)" with "full streaming, not
 * just status" and "Collapsed sub-agent blocks always show tokens used
 * and duration."
 *
 * Visual design:
 * - border-l-2 accent color (blue=running, green=completed, red=failed)
 * - Collapsible body (expanded while running, collapsed when completed)
 * - Header: agent label, status indicator, task description
 * - Footer (always visible): tokens used + duration
 */

import { useState, useEffect, memo } from "react";
import {
  ChevronRight,
  Loader2,
  Check,
  X,
  Ban,
} from "lucide-react";
import { cn } from "@/lib/utils";
import { useAgentStore } from "@/stores/agent-store";
import { MarkdownRenderer } from "@/components/chat/markdown-renderer";
import type { AgentStatus } from "@/types/agent";

interface AgentBlockProps {
  agentId: string;
}

/** Format token count with thousands separator */
function formatTokens(tokens: number): string {
  return tokens.toLocaleString();
}

/** Format duration in milliseconds to human-readable */
function formatDuration(ms: number): string {
  if (ms < 1000) return `${ms}ms`;
  return `${(ms / 1000).toFixed(1)}s`;
}

/** Status indicator icon */
function StatusIcon({ status }: { status: AgentStatus }) {
  switch (status) {
    case "running":
      return <Loader2 className="size-3.5 animate-spin text-blue-400" />;
    case "completed":
      return <Check className="size-3.5 text-green-400" />;
    case "failed":
      return <X className="size-3.5 text-red-400" />;
    case "cancelled":
      return <Ban className="size-3.5 text-yellow-400" />;
    case "pending":
      return (
        <div className="size-3.5 rounded-full border-2 border-muted-foreground/40" />
      );
  }
}

/** Border color based on agent status */
function statusBorderColor(status: AgentStatus): string {
  switch (status) {
    case "running":
      return "border-l-blue-500";
    case "completed":
      return "border-l-green-500";
    case "failed":
      return "border-l-red-500";
    case "cancelled":
      return "border-l-yellow-500";
    case "pending":
      return "border-l-muted-foreground/40";
  }
}

export const AgentBlock = memo(function AgentBlock({
  agentId,
}: AgentBlockProps) {
  const agent = useAgentStore((s) => s.agents.get(agentId));

  // Auto-collapse when completed, auto-expand when running
  const [isOpen, setIsOpen] = useState(true);
  useEffect(() => {
    if (!agent) return;
    if (agent.status === "completed" || agent.status === "cancelled") {
      setIsOpen(false);
    } else if (agent.status === "running") {
      setIsOpen(true);
    }
  }, [agent?.status]);

  if (!agent) return null;

  // Truncate task to ~80 chars for header
  const taskLabel =
    agent.task.length > 80 ? agent.task.slice(0, 77) + "..." : agent.task;

  const agentLabel = `Agent ${agent.index + 1}/${agent.total}`;

  return (
    <div
      className={cn(
        "border-l-2 ml-4 my-2 rounded-r-lg bg-muted/30",
        statusBorderColor(agent.status),
      )}
    >
      {/* Header - always visible, clickable to toggle */}
      <button
        type="button"
        onClick={() => setIsOpen(!isOpen)}
        className="w-full flex items-center gap-2 px-3 py-2 text-left hover:bg-muted/50 transition-colors rounded-tr-lg"
      >
        <ChevronRight
          className={cn(
            "size-3.5 text-muted-foreground transition-transform shrink-0",
            isOpen && "rotate-90",
          )}
        />
        <StatusIcon status={agent.status} />
        <span className="text-xs font-medium text-muted-foreground shrink-0">
          {agentLabel}
        </span>
        <span className="text-xs text-foreground truncate">{taskLabel}</span>
      </button>

      {/* Body - collapsible streamed text */}
      {isOpen && agent.streamedText && (
        <div className="px-4 pb-2 text-sm">
          <MarkdownRenderer content={agent.streamedText} />
        </div>
      )}

      {/* Footer - always visible: tokens + duration */}
      {(agent.tokensUsed > 0 || agent.durationMs > 0) && (
        <div className="px-4 pb-2 flex items-center gap-2 text-xs text-muted-foreground">
          {agent.tokensUsed > 0 && (
            <span>{formatTokens(agent.tokensUsed)} tokens</span>
          )}
          {agent.tokensUsed > 0 && agent.durationMs > 0 && (
            <span className="text-muted-foreground/50">|</span>
          )}
          {agent.durationMs > 0 && (
            <span>{formatDuration(agent.durationMs)}</span>
          )}
        </div>
      )}
    </div>
  );
});
