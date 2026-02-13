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
import type { ChatMessage } from "@/types/chat";
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
    clearStreamedContent,
    streamedContent,
    isStreaming,
    error,
  } = useSSEChat();

  const handleSend = useCallback(
    async (message: string) => {
      // Optimistically add the user's message so it appears immediately
      if (activeSessionId) {
        const optimisticMsg: ChatMessage = {
          id: `optimistic-${Date.now()}`,
          session_id: activeSessionId,
          role: "user",
          content: message,
          created_at: new Date().toISOString(),
          input_tokens: null,
          output_tokens: null,
          model: null,
        };
        queryClient.setQueryData<ChatMessage[]>(
          ["messages", activeSessionId],
          (old) => [...(old ?? []), optimisticMsg],
        );
      }

      // Send to current session, or undefined to create a new one
      const resolvedId = await sendMessage(
        botId,
        message,
        activeSessionId ?? undefined,
      );

      // Use resolvedId (returned from stream) to avoid stale closure issues.
      const sid = resolvedId ?? activeSessionId;
      if (sid) {
        await queryClient.invalidateQueries({
          queryKey: ["messages", sid],
        });
      }
      clearStreamedContent();
      queryClient.invalidateQueries({ queryKey: ["sessions", botId] });
      queryClient.invalidateQueries({ queryKey: ["stats"] });
      queryClient.invalidateQueries({ queryKey: ["bots"] });
      // If a new session was created, select it
      if (!activeSessionId && resolvedId) {
        setSelectedSessionId(resolvedId);
      }
    },
    [botId, activeSessionId, sendMessage, clearStreamedContent, queryClient],
  );

  const handleNewChat = () => {
    setSelectedSessionId(null);
  };

  const hasMultipleSessions = sessions && sessions.length > 1;
  const isChatDisabled = bot != null && bot.status !== "active";

  return (
    <div className="flex flex-col h-[calc(100vh-14rem)]">
      {/* Disabled banner */}
      {isChatDisabled && (
        <div className="mx-3 mt-2 flex items-center gap-2 rounded-md border border-yellow-500/50 bg-yellow-500/10 px-4 py-3 text-sm text-yellow-600 dark:text-yellow-400">
          <AlertCircle className="h-4 w-4 shrink-0" />
          <span>This bot is {bot.status} and cannot chat. Enable it from the dashboard to resume.</span>
        </div>
      )}

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
        disabled={isChatDisabled}
      />
    </div>
  );
}
