import { createFileRoute } from "@tanstack/react-router";
import { useQuery } from "@tanstack/react-query";
import { apiFetch } from "@/lib/api-client";
import type { ChatSession } from "@/types/chat";
import type { SoulVersion } from "@/types/soul";
import { Skeleton } from "@/components/ui/skeleton";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { MessageSquare, History, GitBranch } from "lucide-react";

export const Route = createFileRoute("/bots/$botId/")({
  component: BotOverviewPage,
});

function BotOverviewPage() {
  const { botId } = Route.useParams();

  const { data: sessions, isLoading: sessionsLoading } = useQuery({
    queryKey: ["bot", botId, "sessions"],
    queryFn: () => apiFetch<ChatSession[]>(`/bots/${botId}/sessions`),
  });

  const { data: versions, isLoading: versionsLoading } = useQuery({
    queryKey: ["bot", botId, "soul-versions"],
    queryFn: () => apiFetch<SoulVersion[]>(`/bots/${botId}/soul/versions`),
  });

  const totalMessages =
    sessions?.reduce((sum, s) => sum + s.message_count, 0) ?? 0;

  const isLoading = sessionsLoading || versionsLoading;

  return (
    <div className="space-y-4">
      <div className="grid gap-4 md:grid-cols-3">
        <Card>
          <CardHeader className="flex flex-row items-center justify-between pb-2">
            <CardTitle className="text-sm font-medium text-muted-foreground">
              Sessions
            </CardTitle>
            <History className="h-4 w-4 text-muted-foreground" />
          </CardHeader>
          <CardContent>
            {isLoading ? (
              <Skeleton className="h-8 w-16" />
            ) : (
              <p className="text-2xl font-bold">{sessions?.length ?? 0}</p>
            )}
          </CardContent>
        </Card>
        <Card>
          <CardHeader className="flex flex-row items-center justify-between pb-2">
            <CardTitle className="text-sm font-medium text-muted-foreground">
              Total Messages
            </CardTitle>
            <MessageSquare className="h-4 w-4 text-muted-foreground" />
          </CardHeader>
          <CardContent>
            {isLoading ? (
              <Skeleton className="h-8 w-16" />
            ) : (
              <p className="text-2xl font-bold">{totalMessages}</p>
            )}
          </CardContent>
        </Card>
        <Card>
          <CardHeader className="flex flex-row items-center justify-between pb-2">
            <CardTitle className="text-sm font-medium text-muted-foreground">
              Soul Versions
            </CardTitle>
            <GitBranch className="h-4 w-4 text-muted-foreground" />
          </CardHeader>
          <CardContent>
            {isLoading ? (
              <Skeleton className="h-8 w-16" />
            ) : (
              <p className="text-2xl font-bold">{versions?.length ?? 0}</p>
            )}
          </CardContent>
        </Card>
      </div>

      <Card>
        <CardHeader>
          <CardTitle>Recent Activity</CardTitle>
        </CardHeader>
        <CardContent>
          {isLoading ? (
            <Skeleton className="h-32" />
          ) : sessions && sessions.length > 0 ? (
            <div className="space-y-3">
              {sessions.slice(0, 5).map((session) => (
                <div
                  key={session.id}
                  className="flex items-center justify-between rounded-md border px-4 py-3"
                >
                  <div className="min-w-0 flex-1">
                    <p className="truncate text-sm font-medium">
                      {session.title || "Untitled session"}
                    </p>
                    <p className="text-xs text-muted-foreground">
                      {new Date(session.started_at).toLocaleDateString()} &middot;{" "}
                      {session.message_count} messages
                    </p>
                  </div>
                  <span className="ml-3 shrink-0 text-xs text-muted-foreground">
                    {session.model}
                  </span>
                </div>
              ))}
            </div>
          ) : (
            <p className="text-sm text-muted-foreground">
              No activity yet. Start a chat to see recent sessions here.
            </p>
          )}
        </CardContent>
      </Card>
    </div>
  );
}
