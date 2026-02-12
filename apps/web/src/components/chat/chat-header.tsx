/**
 * Chat session header showing bot info and session actions.
 *
 * Displays: bot emoji, bot name, model badge.
 * Actions: clear messages, delete session.
 */

import { useState } from "react";
import { Trash2, Eraser } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogDescription,
  DialogFooter,
} from "@/components/ui/dialog";
import type { Bot } from "@/types/bot";

interface ChatHeaderProps {
  bot: Bot | undefined;
  model: string | undefined;
  sessionTitle: string | null;
  onDelete: () => void;
  onClear: () => void;
}

/**
 * Extract a readable model display name from the full model string.
 * e.g., "claude-sonnet-4-20250514" -> "Claude Sonnet"
 */
function formatModelName(model: string): string {
  if (model.includes("opus")) return "Claude Opus";
  if (model.includes("sonnet")) return "Claude Sonnet";
  if (model.includes("haiku")) return "Claude Haiku";
  if (model.includes("gpt-4")) return "GPT-4";
  if (model.includes("gpt-3.5")) return "GPT-3.5";
  // Fallback: capitalize first segment
  const parts = model.split(/[-_]/);
  return parts
    .slice(0, 2)
    .map((p) => p.charAt(0).toUpperCase() + p.slice(1))
    .join(" ");
}

export function ChatHeader({
  bot,
  model,
  sessionTitle,
  onDelete,
  onClear,
}: ChatHeaderProps) {
  const [deleteDialogOpen, setDeleteDialogOpen] = useState(false);

  return (
    <>
      <div className="flex items-center justify-between border-b px-4 py-3">
        {/* Bot info */}
        <div className="flex items-center gap-3 min-w-0">
          <span className="text-xl shrink-0">{bot?.emoji || "..."}</span>
          <div className="min-w-0">
            <div className="flex items-center gap-2">
              <h2 className="font-semibold truncate">
                {bot?.name || "Chat"}
              </h2>
              {model && (
                <Badge variant="secondary" className="text-xs shrink-0">
                  {formatModelName(model)}
                </Badge>
              )}
            </div>
            {sessionTitle && (
              <p className="text-xs text-muted-foreground truncate">
                {sessionTitle}
              </p>
            )}
          </div>
        </div>

        {/* Actions */}
        <div className="flex items-center gap-1 shrink-0">
          <Button
            variant="ghost"
            size="icon-sm"
            onClick={onClear}
            title="Clear messages"
          >
            <Eraser className="size-4" />
          </Button>
          <Button
            variant="ghost"
            size="icon-sm"
            onClick={() => setDeleteDialogOpen(true)}
            title="Delete session"
          >
            <Trash2 className="size-4" />
          </Button>
        </div>
      </div>

      {/* Delete confirmation dialog */}
      <Dialog open={deleteDialogOpen} onOpenChange={setDeleteDialogOpen}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>Delete session?</DialogTitle>
            <DialogDescription>
              This will permanently delete this session and all its messages.
              This action cannot be undone.
            </DialogDescription>
          </DialogHeader>
          <DialogFooter>
            <Button
              variant="outline"
              onClick={() => setDeleteDialogOpen(false)}
            >
              Cancel
            </Button>
            <Button
              variant="destructive"
              onClick={() => {
                setDeleteDialogOpen(false);
                onDelete();
              }}
            >
              Delete
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </>
  );
}
