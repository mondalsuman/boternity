/**
 * Custom React Flow node for Sub-Workflow steps.
 *
 * Displays referenced workflow name and status indicator.
 */

import { memo } from "react";
import { Handle, Position } from "@xyflow/react";
import type { NodeProps } from "@xyflow/react";
import { Layers } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { nodeStatusClass } from "./shared";

export interface SubWorkflowNodeData {
  label: string;
  workflow_name: string;
  input?: unknown;
  status?: string;
  [key: string]: unknown;
}

function SubWorkflowNodeComponent({ data }: NodeProps) {
  const d = data as unknown as SubWorkflowNodeData;

  return (
    <div
      className={`rounded-lg border-2 bg-card p-3 shadow-sm min-w-[220px] max-w-[280px] ${nodeStatusClass(d.status)}`}
    >
      <Handle type="target" position={Position.Top} className="!bg-primary" />

      <div className="flex items-center gap-2 mb-2">
        <Layers className="size-4 text-indigo-500 shrink-0" />
        <span className="text-sm font-medium truncate">{d.label}</span>
        <Badge variant="secondary" className="text-[10px] ml-auto shrink-0">
          Sub-Workflow
        </Badge>
      </div>

      <div className="space-y-1">
        <p className="text-xs text-muted-foreground">
          Workflow:{" "}
          <span className="font-medium text-foreground">
            {d.workflow_name}
          </span>
        </p>
        {d.input != null && (
          <p className="text-[10px] text-muted-foreground">
            Has input payload
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

export const SubWorkflowNode = memo(SubWorkflowNodeComponent);
