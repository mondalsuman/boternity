/**
 * Custom React Flow node for Approval Gate steps.
 *
 * Displays prompt preview, yellow accent border, and status indicator.
 */

import { memo } from "react";
import { Handle, Position } from "@xyflow/react";
import type { NodeProps } from "@xyflow/react";
import { UserCheck } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { nodeStatusClass } from "./shared";

export interface ApprovalNodeData {
  label: string;
  prompt: string;
  timeout_secs?: number;
  status?: string;
  [key: string]: unknown;
}

function ApprovalNodeComponent({ data }: NodeProps) {
  const d = data as unknown as ApprovalNodeData;
  const baseStatus = nodeStatusClass(d.status);
  // Always add yellow accent unless status overrides
  const borderClass =
    d.status && d.status !== "pending"
      ? baseStatus
      : "border-yellow-500/60";

  return (
    <div
      className={`rounded-lg border-2 bg-card p-3 shadow-sm min-w-[220px] max-w-[280px] ${borderClass}`}
    >
      <Handle type="target" position={Position.Top} className="!bg-primary" />

      <div className="flex items-center gap-2 mb-2">
        <UserCheck className="size-4 text-yellow-500 shrink-0" />
        <span className="text-sm font-medium truncate">{d.label}</span>
        <Badge
          variant="outline"
          className="text-[10px] ml-auto shrink-0 border-yellow-500/40 text-yellow-500"
        >
          Approval Gate
        </Badge>
      </div>

      <div className="space-y-1">
        <p className="text-xs text-muted-foreground truncate italic">
          "{d.prompt?.slice(0, 60)}"
        </p>
        {d.timeout_secs != null && (
          <p className="text-[10px] text-muted-foreground">
            Timeout: {d.timeout_secs}s
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

export const ApprovalNode = memo(ApprovalNodeComponent);
