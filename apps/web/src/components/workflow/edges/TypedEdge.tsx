/**
 * Custom React Flow edge with color coding by data type.
 *
 * Color scheme:
 * - text: blue (#3b82f6)
 * - json: green (#22c55e)
 * - file: orange (#f97316)
 * - default: gray (#94a3b8)
 *
 * Animated dashed line during execution via CSS.
 * Shows data type label on hover.
 */

import { memo, useState } from "react";
import {
  BaseEdge,
  EdgeLabelRenderer,
  getBezierPath,
} from "@xyflow/react";
import type { EdgeProps } from "@xyflow/react";

/** Map data types to colors. */
const DATA_TYPE_COLORS: Record<string, string> = {
  text: "#3b82f6",
  json: "#22c55e",
  file: "#f97316",
  default: "#94a3b8",
};

function getEdgeColor(dataType?: string): string {
  return DATA_TYPE_COLORS[dataType ?? "default"] ?? DATA_TYPE_COLORS.default;
}

function TypedEdgeComponent({
  id,
  sourceX,
  sourceY,
  targetX,
  targetY,
  sourcePosition,
  targetPosition,
  data,
  markerEnd,
}: EdgeProps) {
  const [hovered, setHovered] = useState(false);
  const edgeData = data as { dataType?: string; animated?: boolean } | undefined;
  const dataType = edgeData?.dataType ?? "default";
  const isAnimated = edgeData?.animated ?? false;
  const color = getEdgeColor(dataType);

  const [edgePath, labelX, labelY] = getBezierPath({
    sourceX,
    sourceY,
    sourcePosition,
    targetX,
    targetY,
    targetPosition,
  });

  return (
    <>
      {/* Invisible wider path for easier hover targeting */}
      <path
        d={edgePath}
        fill="none"
        stroke="transparent"
        strokeWidth={20}
        onMouseEnter={() => setHovered(true)}
        onMouseLeave={() => setHovered(false)}
      />

      <BaseEdge
        id={id}
        path={edgePath}
        markerEnd={markerEnd}
        style={{
          stroke: color,
          strokeWidth: 2,
          strokeDasharray: isAnimated ? "5 5" : undefined,
          animation: isAnimated
            ? "typed-edge-dash 0.5s linear infinite"
            : undefined,
        }}
      />

      {/* Data type label on hover */}
      {hovered && dataType !== "default" && (
        <EdgeLabelRenderer>
          <div
            className="absolute text-[10px] px-1.5 py-0.5 rounded bg-popover border shadow-sm pointer-events-none"
            style={{
              transform: `translate(-50%, -50%) translate(${labelX}px,${labelY}px)`,
              color,
            }}
          >
            {dataType}
          </div>
        </EdgeLabelRenderer>
      )}

      {/* CSS animation for dashed edge */}
      <style>{`
        @keyframes typed-edge-dash {
          to {
            stroke-dashoffset: -10;
          }
        }
      `}</style>
    </>
  );
}

export const TypedEdge = memo(TypedEdgeComponent);
