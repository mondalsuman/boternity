import { useState } from "react";
import { formatDistanceToNow, format } from "date-fns";
import { History, ChevronRight, GitCompare, RotateCcw } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { Skeleton } from "@/components/ui/skeleton";
import { cn } from "@/lib/utils";
import { useSoulVersions } from "@/hooks/use-soul-queries";
import type { SoulVersion } from "@/types/soul";

interface VersionTimelineProps {
  botId: string;
  currentVersion: number;
  open: boolean;
  onToggle: () => void;
  onCompare: (original: SoulVersion, modified: SoulVersion) => void;
  onRestore: (version: SoulVersion) => void;
}

export function VersionTimeline({
  botId,
  currentVersion,
  open,
  onToggle,
  onCompare,
  onRestore,
}: VersionTimelineProps) {
  const { data: versions, isLoading } = useSoulVersions(botId);
  const [selectedVersion, setSelectedVersion] = useState<number | null>(null);

  return (
    <div
      className={cn(
        "border-l bg-background transition-all duration-300 ease-in-out overflow-hidden flex-shrink-0",
        open ? "w-[280px]" : "w-0",
      )}
    >
      <div className="w-[280px] h-full flex flex-col">
        {/* Header */}
        <div className="flex items-center justify-between px-3 py-2 border-b">
          <div className="flex items-center gap-2">
            <History className="size-4 text-muted-foreground" />
            <span className="text-sm font-medium">Version History</span>
          </div>
          <Button
            variant="ghost"
            size="icon-xs"
            onClick={onToggle}
            aria-label="Close version history"
          >
            <ChevronRight className="size-3.5" />
          </Button>
        </div>

        {/* Timeline */}
        <div className="flex-1 overflow-y-auto px-3 py-3">
          {isLoading ? (
            <TimelineSkeleton />
          ) : versions && versions.length > 0 ? (
            <div className="relative">
              {versions.map((version, index) => {
                const isCurrent = version.version === currentVersion;
                const isLast = index === versions.length - 1;
                const isSelected = selectedVersion === version.version;

                return (
                  <div
                    key={version.version}
                    className="relative pl-6 pb-4 last:pb-0"
                  >
                    {/* Connecting line */}
                    {!isLast && (
                      <div className="absolute left-[7px] top-3 bottom-0 w-[2px] bg-muted" />
                    )}

                    {/* Timeline dot */}
                    <div
                      className={cn(
                        "absolute left-[2px] top-[6px] size-3 rounded-full border-2",
                        isCurrent
                          ? "bg-primary border-primary"
                          : "bg-background border-muted-foreground/40",
                      )}
                    />

                    {/* Version card */}
                    <button
                      type="button"
                      onClick={() =>
                        setSelectedVersion(
                          isSelected ? null : version.version,
                        )
                      }
                      className={cn(
                        "w-full text-left rounded-md px-2.5 py-2 transition-colors hover:bg-accent/50",
                        isSelected && "bg-accent",
                      )}
                    >
                      <div className="flex items-center gap-1.5 mb-0.5">
                        <Badge
                          variant={isCurrent ? "default" : "outline"}
                          className="text-[10px] px-1.5 py-0 h-4"
                        >
                          v{version.version}
                        </Badge>
                        {isCurrent && (
                          <span className="text-[10px] text-primary font-medium">
                            current
                          </span>
                        )}
                      </div>
                      <p className="text-xs text-muted-foreground leading-snug">
                        Edited{" "}
                        {format(new Date(version.created_at), "MMM d, yyyy")} at{" "}
                        {format(new Date(version.created_at), "h:mm a")}
                      </p>
                      <p className="text-[10px] text-muted-foreground/70 mt-0.5">
                        {formatDistanceToNow(new Date(version.created_at), {
                          addSuffix: true,
                        })}
                      </p>

                      {/* Actions (visible when selected) */}
                      {isSelected && !isCurrent && (
                        <div className="flex items-center gap-1 mt-2">
                          <Button
                            variant="outline"
                            size="xs"
                            onClick={(e) => {
                              e.stopPropagation();
                              // Find current version object
                              const current = versions.find(
                                (v) => v.version === currentVersion,
                              );
                              if (current) {
                                onCompare(version, current);
                              }
                            }}
                          >
                            <GitCompare className="size-3" />
                            Compare
                          </Button>
                          <Button
                            variant="outline"
                            size="xs"
                            onClick={(e) => {
                              e.stopPropagation();
                              onRestore(version);
                            }}
                          >
                            <RotateCcw className="size-3" />
                            Restore
                          </Button>
                        </div>
                      )}
                    </button>
                  </div>
                );
              })}
            </div>
          ) : (
            <p className="text-xs text-muted-foreground text-center py-4">
              No versions yet.
            </p>
          )}
        </div>
      </div>
    </div>
  );
}

/** Skeleton for loading state. */
function TimelineSkeleton() {
  return (
    <div className="space-y-4">
      {Array.from({ length: 5 }).map((_, i) => (
        <div key={i} className="relative pl-6">
          {i < 4 && (
            <div className="absolute left-[7px] top-3 bottom-0 w-[2px] bg-muted" />
          )}
          <Skeleton className="absolute left-[2px] top-[6px] size-3 rounded-full" />
          <div className="space-y-1.5 py-1">
            <Skeleton className="h-4 w-12" />
            <Skeleton className="h-3 w-36" />
            <Skeleton className="h-3 w-20" />
          </div>
        </div>
      ))}
    </div>
  );
}
