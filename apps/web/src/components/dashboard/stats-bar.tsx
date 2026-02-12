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
        "flex items-center gap-3 rounded-lg border px-4 py-3 text-left transition-colors",
        "hover:bg-accent/50",
        isActive && "border-primary bg-accent",
      )}
    >
      <div className="flex size-9 items-center justify-center rounded-md bg-muted">
        {icon}
      </div>
      <div>
        <p className="text-2xl font-bold tabular-nums">{value}</p>
        <p className="text-xs text-muted-foreground">{label}</p>
      </div>
    </button>
  );
}

function StatsBarSkeleton() {
  return (
    <div className="grid gap-4 md:grid-cols-3">
      {Array.from({ length: 3 }).map((_, i) => (
        <Skeleton key={i} className="h-[72px] rounded-lg" />
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
    <div className="grid gap-4 md:grid-cols-3">
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
