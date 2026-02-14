/**
 * Custom React Flow node for HTTP steps.
 *
 * Displays HTTP method badge, URL, and status indicator.
 */

import { memo } from "react";
import { Handle, Position } from "@xyflow/react";
import type { NodeProps } from "@xyflow/react";
import { Globe } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { nodeStatusClass } from "./shared";

export interface HttpNodeData {
  label: string;
  method: string;
  url: string;
  status?: string;
  [key: string]: unknown;
}

/** Method badge colors. */
function methodColor(method: string): string {
  switch (method.toUpperCase()) {
    case "GET":
      return "bg-blue-500/10 text-blue-500 border-blue-500/20";
    case "POST":
      return "bg-green-500/10 text-green-500 border-green-500/20";
    case "PUT":
      return "bg-yellow-500/10 text-yellow-500 border-yellow-500/20";
    case "DELETE":
      return "bg-red-500/10 text-red-500 border-red-500/20";
    case "PATCH":
      return "bg-orange-500/10 text-orange-500 border-orange-500/20";
    default:
      return "bg-muted text-muted-foreground";
  }
}

function HttpNodeComponent({ data }: NodeProps) {
  const d = data as unknown as HttpNodeData;

  return (
    <div
      className={`rounded-lg border-2 bg-card p-3 shadow-sm min-w-[220px] max-w-[280px] ${nodeStatusClass(d.status)}`}
    >
      <Handle type="target" position={Position.Top} className="!bg-primary" />

      <div className="flex items-center gap-2 mb-2">
        <Globe className="size-4 text-sky-500 shrink-0" />
        <span className="text-sm font-medium truncate">{d.label}</span>
        <Badge variant="secondary" className="text-[10px] ml-auto shrink-0">
          HTTP
        </Badge>
      </div>

      <div className="space-y-1">
        <Badge variant="outline" className={`text-[10px] ${methodColor(d.method)}`}>
          {d.method.toUpperCase()}
        </Badge>
        <p className="text-xs text-muted-foreground truncate">{d.url}</p>
      </div>

      <Handle
        type="source"
        position={Position.Bottom}
        className="!bg-primary"
      />
    </div>
  );
}

export const HttpNode = memo(HttpNodeComponent);
