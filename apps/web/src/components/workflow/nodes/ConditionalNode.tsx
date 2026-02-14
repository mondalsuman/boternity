/**
 * Custom React Flow node for Conditional (if/else) steps.
 *
 * Displays condition preview, with two source handles:
 * - bottom-left for "then" branch
 * - bottom-right for "else" branch
 */

import { memo } from "react";
import { Handle, Position } from "@xyflow/react";
import type { NodeProps } from "@xyflow/react";
import { GitBranch } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { nodeStatusClass } from "./shared";

export interface ConditionalNodeData {
  label: string;
  condition: string;
  then_steps: string[];
  else_steps: string[];
  status?: string;
  [key: string]: unknown;
}

function ConditionalNodeComponent({ data }: NodeProps) {
  const d = data as unknown as ConditionalNodeData;

  return (
    <div
      className={`rounded-lg border-2 bg-card p-3 shadow-sm min-w-[220px] max-w-[280px] ${nodeStatusClass(d.status)}`}
    >
      <Handle type="target" position={Position.Top} className="!bg-primary" />

      <div className="flex items-center gap-2 mb-2">
        <GitBranch className="size-4 text-purple-500 shrink-0" />
        <span className="text-sm font-medium truncate">{d.label}</span>
        <Badge variant="secondary" className="text-[10px] ml-auto shrink-0">
          If/Else
        </Badge>
      </div>

      <div className="space-y-1">
        <p className="text-xs text-muted-foreground font-mono truncate">
          {d.condition?.slice(0, 50)}
        </p>
        <div className="flex justify-between text-[10px] text-muted-foreground">
          <span className="text-green-500">Then: {d.then_steps?.length ?? 0}</span>
          <span className="text-red-400">Else: {d.else_steps?.length ?? 0}</span>
        </div>
      </div>

      {/* Two source handles: then (left) and else (right) */}
      <Handle
        type="source"
        position={Position.Bottom}
        id="then"
        style={{ left: "30%" }}
        className="!bg-green-500"
      />
      <Handle
        type="source"
        position={Position.Bottom}
        id="else"
        style={{ left: "70%" }}
        className="!bg-red-400"
      />
    </div>
  );
}

export const ConditionalNode = memo(ConditionalNodeComponent);
