import { Bot, MessageSquare, Activity } from "lucide-react";
import { useStats } from "@/hooks/use-stats-query";
import { Skeleton } from "@/components/ui/skeleton";
import { cn } from "@/lib/utils";

interface StatsBarProps {
  activeFilter: string | null;
  onFilterChange: (filter: string | null) => void;
}

function StatItem({
  icon,
  label,
  value,
}: {
  icon: React.ReactNode;
  label: string;
  value: number;
}) {
  return (
    <div className="flex items-center gap-2 md:gap-3 rounded-lg border px-2 py-2 md:px-4 md:py-3 text-left">
      <div className="hidden md:flex size-9 items-center justify-center rounded-md bg-muted">
        {icon}
      </div>
      <div className="min-w-0">
        <p className="text-lg md:text-2xl font-bold tabular-nums">{value}</p>
        <p className="text-[10px] md:text-xs text-muted-foreground truncate">{label}</p>
      </div>
    </div>
  );
}

function FilterChip({
  label,
  isActive,
  onClick,
}: {
  label: string;
  isActive: boolean;
  onClick: () => void;
}) {
  return (
    <button
      onClick={onClick}
      className={cn(
        "rounded-full px-3 py-1 text-xs font-medium transition-colors",
        isActive
          ? "bg-primary text-primary-foreground"
          : "bg-muted text-muted-foreground hover:bg-accent hover:text-accent-foreground",
      )}
    >
      {label}
    </button>
  );
}

function StatsBarSkeleton() {
  return (
    <div className="grid grid-cols-3 gap-2 md:gap-4">
      {Array.from({ length: 3 }).map((_, i) => (
        <Skeleton key={i} className="h-[60px] md:h-[72px] rounded-lg" />
      ))}
    </div>
  );
}

export function StatsBar({ activeFilter, onFilterChange }: StatsBarProps) {
  const { data: stats, isLoading } = useStats();

  if (isLoading) {
    return <StatsBarSkeleton />;
  }

  if (!stats) {
    return null;
  }

  return (
    <div className="space-y-3">
      <div className="grid grid-cols-3 gap-2 md:gap-4">
        <StatItem
          icon={<Bot className="size-4 text-muted-foreground" />}
          label="Total Bots"
          value={stats.total_bots}
        />
        <StatItem
          icon={<Activity className="size-4 text-muted-foreground" />}
          label="Active Sessions"
          value={stats.active_sessions}
        />
        <StatItem
          icon={<MessageSquare className="size-4 text-muted-foreground" />}
          label="Total Conversations"
          value={stats.total_messages}
        />
      </div>
      <div className="flex items-center gap-2">
        <span className="text-xs text-muted-foreground mr-1">Show:</span>
        <FilterChip
          label="All Bots"
          isActive={activeFilter === null}
          onClick={() => onFilterChange(null)}
        />
        <FilterChip
          label={`Active (${stats.active_bots})`}
          isActive={activeFilter === "active"}
          onClick={() => onFilterChange(activeFilter === "active" ? null : "active")}
        />
        {stats.disabled_bots > 0 && (
          <FilterChip
            label={`Disabled (${stats.disabled_bots})`}
            isActive={activeFilter === "disabled"}
            onClick={() => onFilterChange(activeFilter === "disabled" ? null : "disabled")}
          />
        )}
        {stats.archived_bots > 0 && (
          <FilterChip
            label={`Archived (${stats.archived_bots})`}
            isActive={activeFilter === "archived"}
            onClick={() => onFilterChange(activeFilter === "archived" ? null : "archived")}
          />
        )}
      </div>
    </div>
  );
}
