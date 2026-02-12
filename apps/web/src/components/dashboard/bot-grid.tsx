import { useMemo, useState } from "react";
import { Search, ArrowUpDown, RefreshCw } from "lucide-react";
import type { Bot, BotStatus } from "@/types/bot";
import { useBots } from "@/hooks/use-bot-queries";
import { BotCard } from "./bot-card";
import { Input } from "@/components/ui/input";
import { Button } from "@/components/ui/button";
import { Skeleton } from "@/components/ui/skeleton";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuRadioGroup,
  DropdownMenuRadioItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";

type SortBy = "name" | "last_activity" | "status";

const SORT_LABELS: Record<SortBy, string> = {
  name: "Name (A-Z)",
  last_activity: "Last Activity",
  status: "Status",
};

const STATUS_ORDER: Record<BotStatus, number> = {
  active: 0,
  disabled: 1,
  archived: 2,
};

interface BotGridProps {
  statusFilter: string | null;
}

function BotGridSkeleton() {
  return (
    <div className="grid gap-4 grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4">
      {Array.from({ length: 6 }).map((_, i) => (
        <Skeleton key={i} className="h-[220px] rounded-xl" />
      ))}
    </div>
  );
}

export function BotGrid({ statusFilter }: BotGridProps) {
  const [searchQuery, setSearchQuery] = useState("");
  const [sortBy, setSortBy] = useState<SortBy>("last_activity");

  const { data: bots, isLoading, isError, refetch } = useBots();

  const filteredAndSorted = useMemo(() => {
    if (!bots) return [];

    let result = [...bots];

    // Filter by status
    if (statusFilter) {
      result = result.filter((b) => b.status === statusFilter);
    }

    // Filter by search query (case-insensitive name match)
    if (searchQuery.trim()) {
      const q = searchQuery.toLowerCase();
      result = result.filter((b) => b.name.toLowerCase().includes(q));
    }

    // Sort
    result.sort((a, b) => {
      switch (sortBy) {
        case "name":
          return a.name.localeCompare(b.name);
        case "last_activity":
          return (
            new Date(b.updated_at).getTime() -
            new Date(a.updated_at).getTime()
          );
        case "status":
          return (
            (STATUS_ORDER[a.status] ?? 9) - (STATUS_ORDER[b.status] ?? 9)
          );
        default:
          return 0;
      }
    });

    return result;
  }, [bots, statusFilter, searchQuery, sortBy]);

  if (isLoading) {
    return <BotGridSkeleton />;
  }

  if (isError) {
    return (
      <div className="flex flex-col items-center justify-center gap-4 py-16 text-center">
        <p className="text-muted-foreground">Failed to load bots.</p>
        <Button variant="outline" onClick={() => refetch()}>
          <RefreshCw className="size-4" />
          Retry
        </Button>
      </div>
    );
  }

  // Let parent handle empty state (0 bots total, not just filtered)
  const isEmpty = !bots || bots.length === 0;

  return (
    <div className="space-y-4">
      {/* Search and sort controls */}
      {!isEmpty && (
        <div className="flex items-center gap-3">
          <div className="relative flex-1 max-w-sm">
            <Search className="absolute left-2.5 top-1/2 size-4 -translate-y-1/2 text-muted-foreground" />
            <Input
              placeholder="Search bots..."
              value={searchQuery}
              onChange={(e) => setSearchQuery(e.target.value)}
              className="pl-8"
            />
          </div>
          <DropdownMenu>
            <DropdownMenuTrigger asChild>
              <Button variant="outline" size="sm">
                <ArrowUpDown className="size-3.5" />
                {SORT_LABELS[sortBy]}
              </Button>
            </DropdownMenuTrigger>
            <DropdownMenuContent align="end">
              <DropdownMenuRadioGroup
                value={sortBy}
                onValueChange={(v) => setSortBy(v as SortBy)}
              >
                {(Object.keys(SORT_LABELS) as SortBy[]).map((key) => (
                  <DropdownMenuRadioItem key={key} value={key}>
                    {SORT_LABELS[key]}
                  </DropdownMenuRadioItem>
                ))}
              </DropdownMenuRadioGroup>
            </DropdownMenuContent>
          </DropdownMenu>
        </div>
      )}

      {/* Grid */}
      {filteredAndSorted.length === 0 && !isEmpty ? (
        <div className="py-12 text-center text-muted-foreground">
          <p>No bots match your search.</p>
        </div>
      ) : (
        <div className="grid gap-4 grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4">
          {filteredAndSorted.map((bot: Bot) => (
            <BotCard key={bot.id} bot={bot} />
          ))}
        </div>
      )}
    </div>
  );
}
