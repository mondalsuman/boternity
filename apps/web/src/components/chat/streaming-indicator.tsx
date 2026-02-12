/**
 * "Bot is thinking..." indicator shown before the first token arrives.
 *
 * Displays when isStreaming is true but no content has been received yet.
 * Uses a simple CSS pulse animation for the dots.
 */

interface StreamingIndicatorProps {
  botEmoji?: string;
}

export function StreamingIndicator({ botEmoji }: StreamingIndicatorProps) {
  return (
    <div className="flex gap-3">
      {/* Bot avatar */}
      <div className="flex-shrink-0 w-8 h-8 rounded-full bg-muted flex items-center justify-center text-sm">
        {botEmoji || "..."}
      </div>

      {/* Thinking bubble */}
      <div className="bg-muted rounded-2xl rounded-bl-md px-4 py-3">
        <div className="flex items-center gap-1.5">
          <span className="text-sm text-muted-foreground">Bot is thinking</span>
          <span className="flex gap-0.5">
            <span
              className="w-1.5 h-1.5 rounded-full bg-muted-foreground/60 animate-bounce"
              style={{ animationDelay: "0ms", animationDuration: "1s" }}
            />
            <span
              className="w-1.5 h-1.5 rounded-full bg-muted-foreground/60 animate-bounce"
              style={{ animationDelay: "150ms", animationDuration: "1s" }}
            />
            <span
              className="w-1.5 h-1.5 rounded-full bg-muted-foreground/60 animate-bounce"
              style={{ animationDelay: "300ms", animationDuration: "1s" }}
            />
          </span>
        </div>
      </div>
    </div>
  );
}
