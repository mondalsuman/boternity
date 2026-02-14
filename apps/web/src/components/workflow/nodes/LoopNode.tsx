/**
 * Custom React Flow node for Loop steps.
 *
 * Displays loop condition, max iterations, and status indicator.
 */

import { memo } from "react";
import { Handle, Position } from "@xyflow/react";
import type { NodeProps } from "@xyflow/react";
import { Repeat } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { nodeStatusClass } from "./shared";

export interface LoopNodeData {
  label: string;
  condition: string;
  max_iterations?: number;
  body_steps: string[];
  status?: string;
  [key: string]: unknown;
}

function LoopNodeComponent({ data }: NodeProps) {
  const d = data as unknown as LoopNodeData;

  return (
    <div
      className={`rounded-lg border-2 bg-card p-3 shadow-sm min-w-[220px] max-w-[280px] ${nodeStatusClass(d.status)}`}
    >
      <Handle type="target" position={Position.Top} className="!bg-primary" />

      <div className="flex items-center gap-2 mb-2">
        <Repeat className="size-4 text-teal-500 shrink-0" />
        <span className="text-sm font-medium truncate">{d.label}</span>
        <Badge variant="secondary" className="text-[10px] ml-auto shrink-0">
          Loop
        </Badge>
      </div>

      <div className="space-y-1">
        <p className="text-xs text-muted-foreground font-mono truncate">
          {d.condition?.slice(0, 50)}
        </p>
        <div className="flex gap-3 text-[10px] text-muted-foreground">
          {d.max_iterations != null && (
            <span>Max: {d.max_iterations}</span>
          )}
          <span>Body: {d.body_steps?.length ?? 0} steps</span>
        </div>
      </div>

      <Handle
        type="source"
        position={Position.Bottom}
        className="!bg-primary"
      />
    </div>
  );
}

export const LoopNode = memo(LoopNodeComponent);
