import { Bot, MessageSquare, Activity } from "lucide-react";
import { useStats } from "@/hooks/use-stats-query";
import { Skeleton } from "@/components/ui/skeleton";
import { cn } from "@/lib/utils";

interface StatsBarProps {
  activeFilter: string | null;
  onFilterChange: (filter: string | null) => void;
}

interface StatItemProps {
  icon: React.ReactNode;
  label: string;
  value: number;
  filterKey: string | null;
  activeFilter: string | null;
  onFilterChange: (filter: string | null) => void;
}

function StatItem({
  icon,
  label,
  value,
  filterKey,
  activeFilter,
  onFilterChange,
}: StatItemProps) {
  const isActive = activeFilter === filterKey;

  return (
    <button
      onClick={() => onFilterChange(isActive ? null : filterKey)}
      className={cn(
        "flex items-center gap-2 md:gap-3 rounded-lg border px-2 py-2 md:px-4 md:py-3 text-left transition-colors",
        "hover:bg-accent/50",
        isActive && "border-primary bg-accent",
      )}
    >
      <div className="hidden md:flex size-9 items-center justify-center rounded-md bg-muted">
        {icon}
      </div>
      <div className="min-w-0">
        <p className="text-lg md:text-2xl font-bold tabular-nums">{value}</p>
        <p className="text-[10px] md:text-xs text-muted-foreground truncate">{label}</p>
      </div>
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
    <div className="grid grid-cols-3 gap-2 md:gap-4">
      <StatItem
        icon={<Bot className="size-4 text-muted-foreground" />}
        label={`Total Bots${stats.active_bots > 0 ? ` (${stats.active_bots} active)` : ""}`}
        value={stats.total_bots}
        filterKey="active"
        activeFilter={activeFilter}
        onFilterChange={onFilterChange}
      />
      <StatItem
        icon={<Activity className="size-4 text-muted-foreground" />}
        label="Active Sessions"
        value={stats.active_sessions}
        filterKey={null}
        activeFilter={activeFilter}
        onFilterChange={onFilterChange}
      />
      <StatItem
        icon={<MessageSquare className="size-4 text-muted-foreground" />}
        label="Total Conversations"
        value={stats.total_messages}
        filterKey={null}
        activeFilter={activeFilter}
        onFilterChange={onFilterChange}
      />
    </div>
  );
}
