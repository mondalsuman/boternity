import { useState } from "react";
import { createFileRoute } from "@tanstack/react-router";
import { Plus } from "lucide-react";
import { useBots } from "@/hooks/use-bot-queries";
import { StatsBar } from "@/components/dashboard/stats-bar";
import { BotGrid } from "@/components/dashboard/bot-grid";
import { Button } from "@/components/ui/button";

export const Route = createFileRoute("/")({
  component: DashboardPage,
});

function DashboardPage() {
  const [statusFilter, setStatusFilter] = useState<string | null>(null);
  const [_createOpen, _setCreateOpen] = useState(false);
  const { data: bots } = useBots();

  const isEmpty = !bots || bots.length === 0;

  return (
    <div className="p-6 space-y-6">
      {/* Header */}
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold tracking-tight">
            Fleet Dashboard
          </h1>
          <p className="text-muted-foreground">
            Overview of all your bots, sessions, and activity.
          </p>
        </div>
        {!isEmpty && (
          <Button className="hidden md:inline-flex" onClick={() => _setCreateOpen(true)}>
            <Plus className="size-4" />
            Create Bot
          </Button>
        )}
      </div>

      {/* Stats bar */}
      {!isEmpty && (
        <StatsBar
          activeFilter={statusFilter}
          onFilterChange={setStatusFilter}
        />
      )}

      {/* Bot grid */}
      <BotGrid statusFilter={statusFilter} />
    </div>
  );
}
