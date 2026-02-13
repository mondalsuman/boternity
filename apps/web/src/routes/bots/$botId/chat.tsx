/**
 * Bot-scoped chat tab -- /bots/$botId/chat
 *
 * Inline chat interface that auto-selects the most recent session
 * for this bot, or starts fresh. Reuses the shared chat components
 * (MessageList, ChatInput) from the global chat feature.
 */

import { useState, useCallback } from "react";
import { createFileRoute } from "@tanstack/react-router";
import { useQueryClient } from "@tanstack/react-query";
import { MessageList } from "@/components/chat/message-list";
import { ChatInput } from "@/components/chat/chat-input";
import { useBotSessions, useMessages } from "@/hooks/use-chat-queries";
import { useBot } from "@/hooks/use-bot-queries";
import { useSSEChat } from "@/hooks/use-sse-chat";
import { Button } from "@/components/ui/button";
import { Plus, AlertCircle } from "lucide-react";

export const Route = createFileRoute("/bots/$botId/chat")({
  component: BotChatPage,
});

function BotChatPage() {
  const { botId } = Route.useParams();
  const queryClient = useQueryClient();

  const { data: bot } = useBot(botId);
  const { data: sessions } = useBotSessions(botId);

  // Track which session is active; null means "new chat"
  const [selectedSessionId, setSelectedSessionId] = useState<string | null>(
    null,
  );

  // Auto-select latest session when sessions load and nothing is selected yet
  const latestSession = sessions?.[0] ?? null;
  const activeSessionId = selectedSessionId ?? latestSession?.id ?? null;

  const { data: messages, isLoading: messagesLoading } =
    useMessages(activeSessionId);

  const {
    sendMessage,
    stopGeneration,
    streamedContent,
    isStreaming,
    activeSessionId: streamSessionId,
    error,
  } = useSSEChat();

  const handleSend = useCallback(
    async (message: string) => {
      // Send to current session, or null to create a new one
      await sendMessage(botId, message, activeSessionId ?? undefined);

      // After streaming completes, refresh data
      queryClient.invalidateQueries({ queryKey: ["sessions", botId] });
      if (activeSessionId) {
        queryClient.invalidateQueries({
          queryKey: ["messages", activeSessionId],
        });
      }
      // If a new session was created, select it
      if (!activeSessionId && streamSessionId) {
        setSelectedSessionId(streamSessionId);
      }
    },
    [botId, activeSessionId, sendMessage, queryClient, streamSessionId],
  );

  // When stream creates a new session, auto-select it
  if (streamSessionId && !activeSessionId) {
    setSelectedSessionId(streamSessionId);
  }

  const handleNewChat = () => {
    setSelectedSessionId(null);
  };

  const hasMultipleSessions = sessions && sessions.length > 1;

  return (
    <div className="flex flex-col h-[calc(100vh-14rem)]">
      {/* Session picker (only shown when there are multiple sessions) */}
      {hasMultipleSessions && (
        <div className="flex items-center gap-2 border-b px-3 py-2 overflow-x-auto">
          <Button
            variant={activeSessionId === null ? "default" : "ghost"}
            size="sm"
            className="shrink-0 gap-1.5"
            onClick={handleNewChat}
          >
            <Plus className="h-3.5 w-3.5" />
            New
          </Button>
          {sessions.slice(0, 10).map((session) => (
            <Button
              key={session.id}
              variant={activeSessionId === session.id ? "secondary" : "ghost"}
              size="sm"
              className="shrink-0 max-w-48 truncate"
              onClick={() => setSelectedSessionId(session.id)}
            >
              {session.title || "Untitled"}
            </Button>
          ))}
        </div>
      )}

      {/* Error banner */}
      {error && (
        <div className="mx-3 mt-2 flex items-center gap-2 rounded-md border border-destructive/50 bg-destructive/10 px-4 py-3 text-sm text-destructive">
          <AlertCircle className="h-4 w-4 shrink-0" />
          <span>{error}</span>
        </div>
      )}

      {/* Messages */}
      <MessageList
        messages={messages}
        isLoading={!!activeSessionId && messagesLoading}
        isStreaming={isStreaming}
        streamedContent={streamedContent}
        botEmoji={bot?.emoji ?? undefined}
      />

      {/* Input */}
      <ChatInput
        onSend={handleSend}
        onStop={stopGeneration}
        isStreaming={isStreaming}
      />
    </div>
  );
}
