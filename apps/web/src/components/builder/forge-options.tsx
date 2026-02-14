/**
 * Interactive option buttons for Forge AskQuestion turns.
 *
 * Renders a vertical list of clickable option buttons with labels
 * and optional descriptions. Clicking an option sends the answer
 * via the WebSocket and adds a user message to the store.
 *
 * Disabled state prevents double-clicks while waiting for a response.
 */

import { cn } from "@/lib/utils";
import type { QuestionOption } from "@/lib/api/builder";

interface ForgeOptionsProps {
  options: QuestionOption[];
  onSelect: (index: number) => void;
  disabled?: boolean;
}

export function ForgeOptions({
  options,
  onSelect,
  disabled = false,
}: ForgeOptionsProps) {
  if (options.length === 0) return null;

  return (
    <div className="flex flex-col gap-2 mt-3">
      {options.map((option, i) => (
        <button
          key={option.id}
          onClick={() => onSelect(i)}
          disabled={disabled}
          className={cn(
            "text-left p-3 rounded-lg border transition-colors",
            disabled
              ? "opacity-50 cursor-not-allowed"
              : "hover:bg-accent hover:border-accent-foreground/20 cursor-pointer",
          )}
        >
          <span className="font-medium text-sm">{option.label}</span>
          {option.description && (
            <span className="block text-sm text-muted-foreground mt-1">
              {option.description}
            </span>
          )}
        </button>
      ))}
    </div>
  );
}
