/**
 * Forge chat message bubble component.
 *
 * Renders Forge (bot builder) and user messages in chat bubble format.
 * Forge messages appear left-aligned with the Forge avatar and support:
 * - Markdown content rendering
 * - Interactive option buttons for AskQuestion turns
 * - "Create Bot" / "Create Skill" action button for ReadyToAssemble turns
 * - Phase label badge for contextual progression
 *
 * User messages appear right-aligned with plain text.
 */

import { memo } from "react";
import { formatDistanceToNow } from "date-fns";
import { Hammer } from "lucide-react";
import { MarkdownRenderer } from "@/components/chat/markdown-renderer";
import { ForgeOptions } from "@/components/builder/forge-options";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { cn } from "@/lib/utils";
import type { ForgeMessage as ForgeMessageType, ForgeMode } from "@/stores/forge-store";
import type { BuilderConfig } from "@/lib/api/builder";

// ---------------------------------------------------------------------------
// Props
// ---------------------------------------------------------------------------

interface ForgeMessageProps {
  message: ForgeMessageType;
  mode: ForgeMode;
  onOptionSelect: (index: number) => void;
  onAssemble?: (config: BuilderConfig) => void;
  onCreateSkill?: () => void;
  isLast?: boolean;
  isWaiting?: boolean;
}

// ---------------------------------------------------------------------------
// Forge avatar
// ---------------------------------------------------------------------------

function ForgeAvatar() {
  return (
    <div className="flex-shrink-0 w-8 h-8 rounded-full bg-amber-500/20 flex items-center justify-center">
      <Hammer className="w-4 h-4 text-amber-500" />
    </div>
  );
}

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

export const ForgeMessage = memo(function ForgeMessage({
  message,
  mode,
  onOptionSelect,
  onAssemble,
  onCreateSkill,
  isLast = false,
  isWaiting = false,
}: ForgeMessageProps) {
  const isUser = message.role === "user";
  const turn = message.turn;

  // Determine if this message has interactive options (only show on last forge message)
  const hasOptions =
    isLast &&
    !isWaiting &&
    turn?.action === "ask_question" &&
    turn.options.length > 0;

  // Determine if this is a ready-to-assemble message
  const isReady = turn?.action === "ready_to_assemble";

  // Extract phase label for AskQuestion turns
  const phaseLabel =
    turn?.action === "ask_question" ? turn.phase_label : undefined;

  return (
    <div
      className={cn("flex gap-3", isUser ? "flex-row-reverse" : "flex-row")}
    >
      {/* Avatar (forge only) */}
      {!isUser && <ForgeAvatar />}

      {/* Message content */}
      <div
        className={cn(
          "flex flex-col max-w-[75%]",
          isUser ? "items-end" : "items-start",
        )}
      >
        {/* Phase label badge */}
        {phaseLabel && (
          <Badge
            variant="outline"
            className="mb-1 text-xs font-normal text-muted-foreground"
          >
            {phaseLabel}
          </Badge>
        )}

        {/* Bubble */}
        <div
          className={cn(
            "rounded-2xl px-4 py-2.5 text-sm leading-relaxed break-words",
            isUser
              ? "bg-primary text-primary-foreground rounded-br-md whitespace-pre-wrap"
              : "bg-muted text-foreground rounded-bl-md",
          )}
        >
          {isUser ? (
            message.content
          ) : (
            <MarkdownRenderer content={message.content} />
          )}
        </div>

        {/* Interactive options for AskQuestion */}
        {hasOptions && turn.action === "ask_question" && (
          <ForgeOptions
            options={turn.options}
            onSelect={onOptionSelect}
            disabled={isWaiting}
          />
        )}

        {/* Assembly / Create button for ReadyToAssemble */}
        {isReady && isLast && !isWaiting && turn.action === "ready_to_assemble" && (
          <div className="mt-3">
            {mode === "skill" ? (
              <Button
                onClick={onCreateSkill}
                className="bg-amber-500 hover:bg-amber-600 text-white"
              >
                Create Skill
              </Button>
            ) : (
              <Button
                onClick={() => onAssemble?.(turn.config)}
                className="bg-amber-500 hover:bg-amber-600 text-white"
              >
                Create Bot
              </Button>
            )}
          </div>
        )}

        {/* Timestamp */}
        <span className="text-xs text-muted-foreground mt-1 px-1">
          {formatDistanceToNow(message.timestamp, { addSuffix: true })}
        </span>
      </div>
    </div>
  );
});
