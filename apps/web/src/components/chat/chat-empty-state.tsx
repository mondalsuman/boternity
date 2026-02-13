/**
 * Chat empty state -- shown when no session is selected.
 *
 * Displays a grid of available bots that the user can click to start a conversation.
 */

import { useNavigate } from "@tanstack/react-router";
import { MessageCircle, Loader2 } from "lucide-react";
import { useBots } from "@/hooks/use-bot-queries";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
} from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Skeleton } from "@/components/ui/skeleton";

export function ChatEmptyState() {
  const { data: bots, isLoading } = useBots();
  const navigate = useNavigate();

  const activeBots = bots?.filter((b) => b.status === "active") ?? [];

  const handleStartChat = (botId: string) => {
    navigate({ to: "/chat", search: { bot: botId } });
  };

  return (
    <div className="flex flex-col items-center justify-center h-full p-6">
      <div className="max-w-2xl w-full space-y-8">
        {/* Header */}
        <div className="text-center space-y-2">
          <MessageCircle className="size-12 mx-auto text-muted-foreground" />
          <h2 className="text-2xl font-semibold tracking-tight">
            Start a conversation
          </h2>
          <p className="text-muted-foreground">
            Choose a bot below to begin chatting.
          </p>
        </div>

        {/* Bot grid */}
        {isLoading ? (
          <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
            {Array.from({ length: 3 }).map((_, i) => (
              <Skeleton key={i} className="h-32 rounded-lg" />
            ))}
          </div>
        ) : activeBots.length === 0 ? (
          <div className="text-center py-8 text-muted-foreground">
            <p>No active bots available.</p>
            <p className="text-sm mt-1">Create a bot first to start chatting.</p>
          </div>
        ) : (
          <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
            {activeBots.map((bot) => (
              <Card
                key={bot.id}
                className="cursor-pointer hover:border-primary/50 transition-colors py-4"
              >
                <CardContent className="flex flex-col items-center text-center gap-3">
                  <span className="text-3xl">{bot.emoji || "..."}</span>
                  <div>
                    <h3 className="font-semibold">{bot.name}</h3>
                    {bot.description && (
                      <p className="text-xs text-muted-foreground mt-0.5 line-clamp-2">
                        {bot.description}
                      </p>
                    )}
                    {bot.category && (
                      <Badge variant="secondary" className="mt-2 text-xs">
                        {bot.category}
                      </Badge>
                    )}
                  </div>
                  <Button
                    size="sm"
                    onClick={() => handleStartChat(bot.id)}
                    className="mt-1"
                  >
                    <MessageCircle className="size-3.5 mr-1.5" />
                    Chat
                  </Button>
                </CardContent>
              </Card>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}
