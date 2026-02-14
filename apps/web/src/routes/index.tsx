import { useState } from "react";
import { createFileRoute, Link } from "@tanstack/react-router";
import { Plus } from "lucide-react";
import { useBots } from "@/hooks/use-bot-queries";
import { StatsBar } from "@/components/dashboard/stats-bar";
import { BotGrid } from "@/components/dashboard/bot-grid";
import { EmptyState } from "@/components/dashboard/empty-state";
import { CreateBotDialog } from "@/components/dashboard/create-bot-dialog";
import { Button } from "@/components/ui/button";

export const Route = createFileRoute("/")({
  component: DashboardPage,
});

function DashboardPage() {
  const [statusFilter, setStatusFilter] = useState<string | null>(null);
  const [createOpen, setCreateOpen] = useState(false);
  const { data: bots, isLoading } = useBots();

  const isEmpty = !isLoading && (!bots || bots.length === 0);

  return (
    <div className="p-4 md:p-6 space-y-4 md:space-y-6">
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
          <Button className="hidden md:inline-flex" asChild>
            <Link to="/builder">
              <Plus className="size-4" />
              Create Bot
            </Link>
          </Button>
        )}
      </div>

      {/* Stats bar (hidden when no bots) */}
      {!isEmpty && (
        <StatsBar
          activeFilter={statusFilter}
          onFilterChange={setStatusFilter}
        />
      )}

      {/* Empty state or bot grid */}
      {isEmpty ? (
        <EmptyState onCreateBot={() => setCreateOpen(true)} />
      ) : (
        <BotGrid statusFilter={statusFilter} />
      )}

      {/* Mobile FAB - visible only on mobile, bottom-right */}
      {!isEmpty && (
        <Button
          size="icon-lg"
          className="fixed bottom-6 right-6 z-40 rounded-full shadow-lg md:hidden safe-bottom"
          asChild
        >
          <Link to="/builder">
            <Plus className="size-5" />
            <span className="sr-only">Create Bot</span>
          </Link>
        </Button>
      )}

      {/* Create bot dialog - kept for quick create from empty state */}
      <CreateBotDialog open={createOpen} onOpenChange={setCreateOpen} />
    </div>
  );
}
