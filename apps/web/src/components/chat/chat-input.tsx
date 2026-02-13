/**
 * Chat message input with auto-expanding textarea.
 *
 * Features:
 * - Auto-expanding textarea (1-6 lines, then scrolls)
 * - Send on Enter (Shift+Enter for newline)
 * - Stop button replaces Send during streaming
 * - Disabled state while streaming
 */

import { useState, useRef, useCallback, type KeyboardEvent } from "react";
import { Send, Square } from "lucide-react";
import { Button } from "@/components/ui/button";

interface ChatInputProps {
  onSend: (message: string) => void;
  onStop: () => void;
  isStreaming: boolean;
  disabled?: boolean;
}

export function ChatInput({
  onSend,
  onStop,
  isStreaming,
  disabled,
}: ChatInputProps) {
  const [value, setValue] = useState("");
  const textareaRef = useRef<HTMLTextAreaElement>(null);

  const adjustHeight = useCallback(() => {
    const textarea = textareaRef.current;
    if (!textarea) return;
    // Reset height to auto to get scrollHeight
    textarea.style.height = "auto";
    // Clamp between 1 line (~40px) and 6 lines (~160px)
    const newHeight = Math.min(textarea.scrollHeight, 160);
    textarea.style.height = `${newHeight}px`;
  }, []);

  const handleSend = useCallback(() => {
    const trimmed = value.trim();
    if (!trimmed || isStreaming) return;
    onSend(trimmed);
    setValue("");
    // Reset textarea height
    if (textareaRef.current) {
      textareaRef.current.style.height = "auto";
    }
  }, [value, isStreaming, onSend]);

  const handleKeyDown = useCallback(
    (e: KeyboardEvent<HTMLTextAreaElement>) => {
      if (e.key === "Enter" && !e.shiftKey) {
        e.preventDefault();
        handleSend();
      }
    },
    [handleSend],
  );

  const handleChange = useCallback(
    (e: React.ChangeEvent<HTMLTextAreaElement>) => {
      setValue(e.target.value);
      adjustHeight();
    },
    [adjustHeight],
  );

  return (
    <div className="border-t bg-background p-3 md:p-4 safe-bottom">
      <div className="flex items-end gap-2 max-w-4xl mx-auto">
        <textarea
          ref={textareaRef}
          value={value}
          onChange={handleChange}
          onKeyDown={handleKeyDown}
          placeholder="Type a message..."
          disabled={isStreaming || disabled}
          rows={1}
          className="flex-1 resize-none rounded-xl border bg-muted/50 px-4 py-3 text-sm placeholder:text-muted-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring disabled:cursor-not-allowed disabled:opacity-50 overflow-y-auto"
          style={{ minHeight: "44px", maxHeight: "160px" }}
        />

        {isStreaming ? (
          <Button
            onClick={onStop}
            size="icon"
            variant="destructive"
            className="shrink-0 rounded-xl"
            title="Stop generating"
          >
            <Square className="size-4" />
          </Button>
        ) : (
          <Button
            onClick={handleSend}
            size="icon"
            disabled={!value.trim() || disabled}
            className="shrink-0 rounded-xl"
            title="Send message"
          >
            <Send className="size-4" />
          </Button>
        )}
      </div>
    </div>
  );
}
