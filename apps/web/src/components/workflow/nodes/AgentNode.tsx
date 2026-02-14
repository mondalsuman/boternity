/**
 * Custom React Flow node for Agent steps.
 *
 * Displays bot name, first line of prompt, and status-colored background.
 */

import { memo } from "react";
import { Handle, Position } from "@xyflow/react";
import type { NodeProps } from "@xyflow/react";
import { Bot } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { nodeStatusClass } from "./shared";

export interface AgentNodeData {
  label: string;
  bot: string;
  prompt: string;
  model?: string;
  status?: string;
  [key: string]: unknown;
}

function AgentNodeComponent({ data }: NodeProps) {
  const d = data as unknown as AgentNodeData;
  const firstLine = d.prompt?.split("\n")[0]?.slice(0, 60) ?? "";

  return (
    <div
      className={`rounded-lg border-2 bg-card p-3 shadow-sm min-w-[220px] max-w-[280px] ${nodeStatusClass(d.status)}`}
    >
      <Handle type="target" position={Position.Top} className="!bg-primary" />

      <div className="flex items-center gap-2 mb-2">
        <Bot className="size-4 text-violet-500 shrink-0" />
        <span className="text-sm font-medium truncate">{d.label}</span>
        <Badge variant="secondary" className="text-[10px] ml-auto shrink-0">
          Agent
        </Badge>
      </div>

      <div className="space-y-1">
        <p className="text-xs text-muted-foreground">
          Bot: <span className="font-medium text-foreground">{d.bot}</span>
        </p>
        {firstLine && (
          <p className="text-xs text-muted-foreground truncate italic">
            "{firstLine}"
          </p>
        )}
      </div>

      <Handle
        type="source"
        position={Position.Bottom}
        className="!bg-primary"
      />
    </div>
  );
}

export const AgentNode = memo(AgentNodeComponent);
