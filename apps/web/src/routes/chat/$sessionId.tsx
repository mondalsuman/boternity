/**
 * Chat session route -- /chat/$sessionId
 *
 * Full chat view: header, message list, input.
 * Wires up SSE streaming for real-time bot responses.
 *
 * Lifecycle: user types -> sendMessage() -> SSE stream starts ->
 * tokens appear live -> stream completes -> invalidate messages query
 * to refresh from server-saved messages.
 */

import { useCallback } from "react";
import { createFileRoute, useNavigate } from "@tanstack/react-router";
import { useQueryClient } from "@tanstack/react-query";
import type { ChatMessage } from "@/types/chat";
import { ChatLayout } from "@/components/chat/chat-layout";
import { ChatHeader } from "@/components/chat/chat-header";
import { MessageList } from "@/components/chat/message-list";
import { ChatInput } from "@/components/chat/chat-input";
import { AlertCircle } from "lucide-react";
import {
  useSession,
  useMessages,
  useDeleteSession,
  useClearSession,
} from "@/hooks/use-chat-queries";
import { useBot } from "@/hooks/use-bot-queries";
import { useSSEChat } from "@/hooks/use-sse-chat";

export const Route = createFileRoute("/chat/$sessionId")({
  component: ChatSessionPage,
});

function ChatSessionPage() {
  const { sessionId } = Route.useParams();
  const navigate = useNavigate();
  const queryClient = useQueryClient();

  // Data queries
  const { data: session } = useSession(sessionId);
  const botId = session?.bot_id ?? null;
  const { data: bot } = useBot(botId ?? "");
  const { data: messages, isLoading: messagesLoading } = useMessages(sessionId);

  // Mutations
  const deleteSession = useDeleteSession();
  const clearSession = useClearSession();

  // SSE streaming
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
      if (!botId) return;

      // Optimistically add the user's message so it appears immediately
      const optimisticMsg: ChatMessage = {
        id: `optimistic-${Date.now()}`,
        session_id: sessionId,
        role: "user",
        content: message,
        created_at: new Date().toISOString(),
        input_tokens: null,
        output_tokens: null,
        model: null,
      };
      queryClient.setQueryData<ChatMessage[]>(
        ["messages", sessionId],
        (old) => [...(old ?? []), optimisticMsg],
      );

      const resolvedId = await sendMessage(botId, message, sessionId);

      // After streaming completes, refresh messages from server.
      // Use resolvedId (returned from stream) to avoid stale closure issues.
      const sid = resolvedId ?? sessionId;
      await queryClient.invalidateQueries({ queryKey: ["messages", sid] });
      clearStreamedContent();
      queryClient.invalidateQueries({ queryKey: ["sessions"] });
      queryClient.invalidateQueries({ queryKey: ["session", sid] });
      queryClient.invalidateQueries({ queryKey: ["stats"] });
      queryClient.invalidateQueries({ queryKey: ["bots"] });
    },
    [botId, sessionId, sendMessage, clearStreamedContent, queryClient],
  );

  const handleDelete = useCallback(() => {
    deleteSession.mutate(sessionId, {
      onSuccess: () => {
        navigate({ to: "/chat" });
      },
    });
  }, [deleteSession, sessionId, navigate]);

  const handleClear = useCallback(() => {
    clearSession.mutate(sessionId);
  }, [clearSession, sessionId]);

  return (
    <ChatLayout activeSessionId={sessionId}>
      <div className="flex flex-col h-full">
        {/* Header */}
        <ChatHeader
          bot={bot}
          model={session?.model}
          sessionTitle={session?.title ?? null}
          onDelete={handleDelete}
          onClear={handleClear}
        />

        {/* Error banner */}
        {error && (
          <div className="mx-4 mt-2 flex items-center gap-2 rounded-md border border-destructive/50 bg-destructive/10 px-4 py-3 text-sm text-destructive">
            <AlertCircle className="h-4 w-4 shrink-0" />
            <span>{error}</span>
          </div>
        )}

        {/* Messages */}
        <MessageList
          messages={messages}
          isLoading={messagesLoading}
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
    </ChatLayout>
  );
}
