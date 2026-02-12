import { createFileRoute, Link, Outlet, useMatchRoute } from "@tanstack/react-router";
import { useQuery } from "@tanstack/react-query";
import { apiFetch } from "@/lib/api-client";
import type { Bot } from "@/types/bot";
import { Tabs, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { Skeleton } from "@/components/ui/skeleton";
import { Badge } from "@/components/ui/badge";

export const Route = createFileRoute("/bots/$botId")({
  component: BotDetailLayout,
});

const STATUS_COLORS: Record<string, string> = {
  active: "bg-green-500",
  disabled: "bg-yellow-500",
  archived: "bg-red-500",
};

/** Derive active tab name from the current route match. */
function useActiveTab(botId: string): string {
  const matchRoute = useMatchRoute();
  if (matchRoute({ to: "/bots/$botId/chat", params: { botId } })) return "chat";
  if (matchRoute({ to: "/bots/$botId/soul", params: { botId } })) return "soul";
  if (matchRoute({ to: "/bots/$botId/settings", params: { botId } })) return "settings";
  return "overview";
}

function BotDetailLayout() {
  const { botId } = Route.useParams();
  const activeTab = useActiveTab(botId);

  const { data: bot, isLoading } = useQuery({
    queryKey: ["bot", botId],
    queryFn: () => apiFetch<Bot>(`/bots/${botId}`),
  });

  if (isLoading) {
    return (
      <div className="p-6 space-y-4">
        <Skeleton className="h-10 w-64" />
        <Skeleton className="h-8 w-96" />
        <Skeleton className="h-[400px]" />
      </div>
    );
  }

  return (
    <div className="p-6 space-y-4">
      {/* Bot header */}
      <div className="flex items-center gap-3">
        <span className="text-3xl">{bot?.emoji || "🤖"}</span>
        <div>
          <div className="flex items-center gap-2">
            <h1 className="text-2xl font-bold tracking-tight">
              {bot?.name || "Bot"}
            </h1>
            {bot && (
              <Badge variant="outline" className="gap-1.5">
                <span
                  className={`w-2 h-2 rounded-full ${STATUS_COLORS[bot.status] || ""}`}
                />
                {bot.status}
              </Badge>
            )}
          </div>
          {bot?.description && (
            <p className="text-muted-foreground">{bot.description}</p>
          )}
        </div>
      </div>

      {/* Tab navigation - value tracks current route */}
      <Tabs
        value={activeTab}
        className="w-full"
      >
        <TabsList>
          <TabsTrigger value="overview" asChild>
            <Link to="/bots/$botId" params={{ botId }}>
              Overview
            </Link>
          </TabsTrigger>
          <TabsTrigger value="chat" asChild>
            <Link to="/bots/$botId/chat" params={{ botId }}>
              Chat
            </Link>
          </TabsTrigger>
          <TabsTrigger value="soul" asChild>
            <Link to="/bots/$botId/soul" params={{ botId }}>
              Soul
            </Link>
          </TabsTrigger>
          <TabsTrigger value="settings" asChild>
            <Link to="/bots/$botId/settings" params={{ botId }}>
              Settings
            </Link>
          </TabsTrigger>
        </TabsList>
      </Tabs>

      {/* Tab content */}
      <Outlet />
    </div>
  );
}
