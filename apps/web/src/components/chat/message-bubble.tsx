/**
 * Individual chat message bubble component.
 *
 * User messages: right-aligned with primary background.
 * Assistant messages: left-aligned with muted background and bot emoji avatar.
 * Shows relative timestamps below each message.
 * Wrapped in React.memo to prevent unnecessary re-renders during streaming.
 */

import { memo } from "react";
import { formatDistanceToNow } from "date-fns";
import type { ChatMessage } from "@/types/chat";

interface MessageBubbleProps {
  message: ChatMessage;
  botEmoji?: string;
}

export const MessageBubble = memo(function MessageBubble({
  message,
  botEmoji,
}: MessageBubbleProps) {
  const isUser = message.role === "user";

  return (
    <div
      className={`flex gap-3 ${isUser ? "flex-row-reverse" : "flex-row"}`}
    >
      {/* Avatar */}
      {!isUser && (
        <div className="flex-shrink-0 w-8 h-8 rounded-full bg-muted flex items-center justify-center text-sm">
          {botEmoji || "..."}
        </div>
      )}

      {/* Message content */}
      <div
        className={`flex flex-col ${isUser ? "items-end" : "items-start"} max-w-[75%]`}
      >
        <div
          className={`rounded-2xl px-4 py-2.5 text-sm leading-relaxed whitespace-pre-wrap break-words ${
            isUser
              ? "bg-primary text-primary-foreground rounded-br-md"
              : "bg-muted text-foreground rounded-bl-md"
          }`}
        >
          {message.content}
        </div>
        <span className="text-xs text-muted-foreground mt-1 px-1">
          {formatDistanceToNow(new Date(message.created_at), {
            addSuffix: true,
          })}
        </span>
      </div>
    </div>
  );
});
