/**
 * Custom React Flow node for Code steps.
 *
 * Displays language badge (TS/WASM), first line of source, and status.
 */

import { memo } from "react";
import { Handle, Position } from "@xyflow/react";
import type { NodeProps } from "@xyflow/react";
import { Code } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { nodeStatusClass } from "./shared";

export interface CodeNodeData {
  label: string;
  language: string;
  source: string;
  status?: string;
  [key: string]: unknown;
}

function CodeNodeComponent({ data }: NodeProps) {
  const d = data as unknown as CodeNodeData;
  const firstLine = d.source?.split("\n")[0]?.slice(0, 50) ?? "";
  const langLabel = d.language === "type_script" ? "TS" : "WASM";

  return (
    <div
      className={`rounded-lg border-2 bg-card p-3 shadow-sm min-w-[220px] max-w-[280px] ${nodeStatusClass(d.status)}`}
    >
      <Handle type="target" position={Position.Top} className="!bg-primary" />

      <div className="flex items-center gap-2 mb-2">
        <Code className="size-4 text-emerald-500 shrink-0" />
        <span className="text-sm font-medium truncate">{d.label}</span>
        <Badge variant="secondary" className="text-[10px] ml-auto shrink-0">
          Code
        </Badge>
      </div>

      <div className="space-y-1">
        <Badge variant="outline" className="text-[10px]">
          {langLabel}
        </Badge>
        {firstLine && (
          <p className="text-xs text-muted-foreground font-mono truncate">
            {firstLine}
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

export const CodeNode = memo(CodeNodeComponent);
