/**
 * WebSocket connection status indicator.
 *
 * Per user decision: "WebSocket connection status indicator visible in
 * web UI (Connected / Reconnecting)."
 *
 * Small dot + optional text indicator:
 * - Connected: green dot
 * - Reconnecting: yellow dot with pulse animation
 * - Disconnected: red dot
 *
 * Designed to be positioned in the chat header area (non-intrusive).
 */

import { memo } from "react";
import { cn } from "@/lib/utils";
import type { WsConnectionStatus } from "@/types/agent";

interface WsStatusProps {
  status: WsConnectionStatus;
  /** Show text label alongside the dot. Default: false (dot only) */
  showLabel?: boolean;
}

const statusConfig: Record<
  WsConnectionStatus,
  { color: string; label: string; pulse: boolean }
> = {
  connected: {
    color: "bg-green-500",
    label: "Connected",
    pulse: false,
  },
  reconnecting: {
    color: "bg-yellow-500",
    label: "Reconnecting...",
    pulse: true,
  },
  disconnected: {
    color: "bg-red-500",
    label: "Disconnected",
    pulse: false,
  },
};

export const WsStatus = memo(function WsStatus({
  status,
  showLabel = false,
}: WsStatusProps) {
  const config = statusConfig[status];

  return (
    <div className="flex items-center gap-1.5" title={config.label}>
      <span className="relative flex size-2">
        {config.pulse && (
          <span
            className={cn(
              "absolute inset-0 rounded-full animate-ping opacity-75",
              config.color,
            )}
          />
        )}
        <span className={cn("relative rounded-full size-2", config.color)} />
      </span>
      {showLabel && (
        <span className="text-[10px] text-muted-foreground">
          {config.label}
        </span>
      )}
    </div>
  );
});
