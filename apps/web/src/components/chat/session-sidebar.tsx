/**
 * Chat session sidebar -- lists sessions grouped by bot name.
 *
 * Like Slack channels: bold bot name headers with sessions listed below.
 * Supports new session creation, delete, and clear actions.
 */

import { useState } from "react";
import { Link, useNavigate } from "@tanstack/react-router";
import { formatDistanceToNow } from "date-fns";
import {
  Plus,
  MessageCircle,
  Trash2,
  Eraser,
  MoreHorizontal,
} from "lucide-react";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Button } from "@/components/ui/button";
import { Skeleton } from "@/components/ui/skeleton";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogDescription,
} from "@/components/ui/dialog";
import {
  useAllSessions,
  useDeleteSession,
  useClearSession,
} from "@/hooks/use-chat-queries";
import type { Bot } from "@/types/bot";

interface SessionSidebarProps {
  activeSessionId?: string;
}

export function SessionSidebar({ activeSessionId }: SessionSidebarProps) {
  const { groups, bots, isLoading } = useAllSessions();
  const deleteSession = useDeleteSession();
  const clearSession = useClearSession();
  const navigate = useNavigate();
  const [botPickerOpen, setBotPickerOpen] = useState(false);

  const handleNewChat = (botId: string) => {
    // Navigate to chat index with botId param to trigger new session
    navigate({ to: "/chat", search: { bot: botId } });
    setBotPickerOpen(false);
  };

  const handleDeleteSession = (sessionId: string) => {
    deleteSession.mutate(sessionId, {
      onSuccess: () => {
        if (activeSessionId === sessionId) {
          navigate({ to: "/chat" });
        }
      },
    });
  };

  const handleClearSession = (sessionId: string) => {
    clearSession.mutate(sessionId);
  };

  if (isLoading) {
    return (
      <div className="flex flex-col h-full">
        <div className="flex items-center justify-between px-4 py-3 border-b">
          <span className="font-semibold text-sm">Sessions</span>
        </div>
        <div className="p-3 space-y-3">
          {Array.from({ length: 5 }).map((_, i) => (
            <div key={i} className="space-y-1.5">
              <Skeleton className="h-4 w-24" />
              <Skeleton className="h-9 w-full" />
            </div>
          ))}
        </div>
      </div>
    );
  }

  return (
    <div className="flex flex-col h-full">
      {/* Header with new chat button */}
      <div className="flex items-center justify-between px-4 py-3 border-b">
        <span className="font-semibold text-sm">Sessions</span>
        <Button
          variant="ghost"
          size="icon-xs"
          onClick={() => setBotPickerOpen(true)}
          title="New chat"
        >
          <Plus className="size-4" />
        </Button>
      </div>

      {/* Session list grouped by bot */}
      <ScrollArea className="flex-1">
        <div className="p-2 space-y-3">
          {groups.length === 0 && (
            <div className="px-2 py-8 text-center text-sm text-muted-foreground">
              <MessageCircle className="size-8 mx-auto mb-2 opacity-50" />
              <p>No sessions yet</p>
              <p className="text-xs mt-1">Start a conversation below</p>
            </div>
          )}

          {groups.map(({ bot, sessions }) => (
            <div key={bot.id}>
              {/* Bot name header */}
              <div className="flex items-center justify-between px-2 py-1">
                <span className="text-xs font-semibold text-muted-foreground uppercase tracking-wider truncate">
                  {bot.emoji || "..."} {bot.name}
                </span>
                <Button
                  variant="ghost"
                  size="icon-xs"
                  onClick={() => handleNewChat(bot.id)}
                  title={`New chat with ${bot.name}`}
                  className="opacity-0 group-hover:opacity-100 hover:opacity-100"
                >
                  <Plus className="size-3" />
                </Button>
              </div>

              {/* Sessions for this bot */}
              <div className="space-y-0.5">
                {sessions.map((session) => (
                  <div
                    key={session.id}
                    className={`group flex items-center gap-1 rounded-md px-2 py-1.5 text-sm hover:bg-accent/50 transition-colors ${
                      activeSessionId === session.id
                        ? "bg-accent text-accent-foreground"
                        : ""
                    }`}
                  >
                    <Link
                      to="/chat/$sessionId"
                      params={{ sessionId: session.id }}
                      className="flex-1 min-w-0"
                    >
                      <div className="flex items-center gap-2">
                        <span className="truncate text-sm">
                          {session.title || "Untitled"}
                        </span>
                      </div>
                      <div className="flex items-center gap-2 text-xs text-muted-foreground">
                        <span>
                          {formatDistanceToNow(new Date(session.started_at), {
                            addSuffix: true,
                          })}
                        </span>
                        <span className="text-muted-foreground/60">
                          {session.message_count} msgs
                        </span>
                      </div>
                    </Link>

                    {/* Session actions */}
                    <DropdownMenu>
                      <DropdownMenuTrigger asChild>
                        <Button
                          variant="ghost"
                          size="icon-xs"
                          className="opacity-0 group-hover:opacity-100 shrink-0"
                        >
                          <MoreHorizontal className="size-3" />
                        </Button>
                      </DropdownMenuTrigger>
                      <DropdownMenuContent align="end" className="w-40">
                        <DropdownMenuItem
                          onClick={() => handleClearSession(session.id)}
                        >
                          <Eraser className="size-4 mr-2" />
                          Clear messages
                        </DropdownMenuItem>
                        <DropdownMenuItem
                          variant="destructive"
                          onClick={() => handleDeleteSession(session.id)}
                        >
                          <Trash2 className="size-4 mr-2" />
                          Delete session
                        </DropdownMenuItem>
                      </DropdownMenuContent>
                    </DropdownMenu>
                  </div>
                ))}
              </div>
            </div>
          ))}
        </div>
      </ScrollArea>

      {/* Bot picker dialog for new chat */}
      <BotPickerDialog
        open={botPickerOpen}
        onOpenChange={setBotPickerOpen}
        bots={bots}
        onSelectBot={handleNewChat}
      />
    </div>
  );
}

/** Dialog to select a bot for a new chat session */
function BotPickerDialog({
  open,
  onOpenChange,
  bots,
  onSelectBot,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  bots: Bot[];
  onSelectBot: (botId: string) => void;
}) {
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-sm">
        <DialogHeader>
          <DialogTitle>New Chat</DialogTitle>
          <DialogDescription>
            Select a bot to start a conversation.
          </DialogDescription>
        </DialogHeader>
        <div className="grid gap-2 max-h-64 overflow-y-auto">
          {bots.length === 0 && (
            <p className="text-sm text-muted-foreground text-center py-4">
              No bots available. Create one first.
            </p>
          )}
          {bots
            .filter((b) => b.status === "active")
            .map((bot) => (
              <Button
                key={bot.id}
                variant="ghost"
                className="justify-start gap-3 h-auto py-3"
                onClick={() => onSelectBot(bot.id)}
              >
                <span className="text-xl">{bot.emoji || "..."}</span>
                <div className="text-left">
                  <div className="font-medium">{bot.name}</div>
                  {bot.description && (
                    <div className="text-xs text-muted-foreground truncate max-w-[200px]">
                      {bot.description}
                    </div>
                  )}
                </div>
              </Button>
            ))}
        </div>
      </DialogContent>
    </Dialog>
  );
}
