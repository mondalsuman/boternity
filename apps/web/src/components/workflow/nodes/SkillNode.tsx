/**
 * Custom React Flow node for Skill steps.
 *
 * Displays skill name, input preview, and status indicator.
 */

import { memo } from "react";
import { Handle, Position } from "@xyflow/react";
import type { NodeProps } from "@xyflow/react";
import { Zap } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { nodeStatusClass } from "./shared";

export interface SkillNodeData {
  label: string;
  skill: string;
  input?: string;
  status?: string;
  [key: string]: unknown;
}

function SkillNodeComponent({ data }: NodeProps) {
  const d = data as unknown as SkillNodeData;

  return (
    <div
      className={`rounded-lg border-2 bg-card p-3 shadow-sm min-w-[220px] max-w-[280px] ${nodeStatusClass(d.status)}`}
    >
      <Handle type="target" position={Position.Top} className="!bg-primary" />

      <div className="flex items-center gap-2 mb-2">
        <Zap className="size-4 text-amber-500 shrink-0" />
        <span className="text-sm font-medium truncate">{d.label}</span>
        <Badge variant="secondary" className="text-[10px] ml-auto shrink-0">
          Skill
        </Badge>
      </div>

      <div className="space-y-1">
        <p className="text-xs text-muted-foreground">
          Skill:{" "}
          <span className="font-medium text-foreground">{d.skill}</span>
        </p>
        {d.input && (
          <p className="text-xs text-muted-foreground truncate italic">
            {d.input.slice(0, 50)}
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

export const SkillNode = memo(SkillNodeComponent);
