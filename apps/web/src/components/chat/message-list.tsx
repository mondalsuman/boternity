/**
 * Scrollable message list with streaming message support.
 *
 * Renders historical messages from the server and a streaming message
 * at the bottom when the bot is actively generating a response.
 * Auto-scrolls to the bottom on new messages.
 *
 * StreamingMessage is its OWN component with its OWN state to prevent
 * the entire message list from re-rendering on each token delta.
 */

import { useEffect, useRef } from "react";
import { MessageCircle } from "lucide-react";
import { Skeleton } from "@/components/ui/skeleton";
import { ScrollArea } from "@/components/ui/scroll-area";
import { MessageBubble } from "@/components/chat/message-bubble";
import { StreamingIndicator } from "@/components/chat/streaming-indicator";
import type { ChatMessage } from "@/types/chat";

interface MessageListProps {
  messages: ChatMessage[] | undefined;
  isLoading: boolean;
  isStreaming: boolean;
  streamedContent: string;
  botEmoji?: string;
}

export function MessageList({
  messages,
  isLoading,
  isStreaming,
  streamedContent,
  botEmoji,
}: MessageListProps) {
  const bottomRef = useRef<HTMLDivElement>(null);

  // Auto-scroll to bottom when messages change or streaming content arrives
  useEffect(() => {
    bottomRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [messages?.length, streamedContent, isStreaming]);

  if (isLoading) {
    return (
      <div className="flex-1 p-4 space-y-4">
        {Array.from({ length: 3 }).map((_, i) => (
          <div
            key={i}
            className={`flex gap-3 ${i % 2 === 1 ? "flex-row-reverse" : ""}`}
          >
            {i % 2 === 0 && <Skeleton className="w-8 h-8 rounded-full" />}
            <Skeleton
              className={`h-16 rounded-2xl ${i % 2 === 1 ? "w-2/3" : "w-3/4"}`}
            />
          </div>
        ))}
      </div>
    );
  }

  const hasMessages = messages && messages.length > 0;
  const showEmptyState = !hasMessages && !isStreaming;

  return (
    <ScrollArea className="flex-1">
      <div className="p-4 space-y-4">
        {/* Empty state for new session */}
        {showEmptyState && (
          <div className="flex flex-col items-center justify-center py-16 text-center">
            <MessageCircle className="size-10 text-muted-foreground mb-3 opacity-50" />
            <p className="text-sm text-muted-foreground">
              Send a message to get started
            </p>
          </div>
        )}

        {/* Historical messages */}
        {messages?.map((msg) => (
          <MessageBubble key={msg.id} message={msg} botEmoji={botEmoji} />
        ))}

        {/* Streaming: thinking indicator or live content */}
        {isStreaming && !streamedContent && (
          <StreamingIndicator botEmoji={botEmoji} />
        )}
        {isStreaming && streamedContent && (
          <StreamingMessage content={streamedContent} botEmoji={botEmoji} />
        )}

        {/* Scroll anchor */}
        <div ref={bottomRef} />
      </div>
    </ScrollArea>
  );
}

/**
 * Isolated streaming message component.
 *
 * Receives streamedContent as a prop and renders it. By being a separate
 * component, React can update just this node on each token delta without
 * re-rendering the entire message list (Pitfall 4 mitigation).
 */
function StreamingMessage({
  content,
  botEmoji,
}: {
  content: string;
  botEmoji?: string;
}) {
  return (
    <div className="flex gap-3">
      <div className="flex-shrink-0 w-8 h-8 rounded-full bg-muted flex items-center justify-center text-sm">
        {botEmoji || "..."}
      </div>
      <div className="flex flex-col items-start max-w-[75%]">
        <div className="bg-muted text-foreground rounded-2xl rounded-bl-md px-4 py-2.5 text-sm leading-relaxed whitespace-pre-wrap break-words">
          {content}
          <span className="inline-block w-0.5 h-4 bg-foreground/70 ml-0.5 animate-pulse align-text-bottom" />
        </div>
      </div>
    </div>
  );
}
