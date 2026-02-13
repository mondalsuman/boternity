/**
 * Chat hub route -- /chat
 *
 * Shows the session sidebar and the bot-grid empty state.
 * Supports ?bot= search param to auto-start a new chat with a bot.
 */

import { createFileRoute } from "@tanstack/react-router";
import { ChatLayout } from "@/components/chat/chat-layout";
import { ChatEmptyState } from "@/components/chat/chat-empty-state";

export const Route = createFileRoute("/chat/")({
  component: ChatHubPage,
  validateSearch: (search: Record<string, unknown>) => ({
    bot: (search.bot as string) || undefined,
  }),
});

function ChatHubPage() {
  return (
    <ChatLayout>
      <ChatEmptyState />
    </ChatLayout>
  );
}
