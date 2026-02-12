import { useState } from "react";
import { useNavigate } from "@tanstack/react-router";
import { Loader2 } from "lucide-react";
import { useCreateBot } from "@/hooks/use-bot-queries";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";

const EMOJI_OPTIONS = [
  "🤖", "🧠", "💡", "🎯", "🔥", "⚡", "🌟", "🎨",
  "📚", "🛠️", "🎭", "🧪", "🚀", "💬", "🐱", "🦊",
];

interface CreateBotDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

export function CreateBotDialog({ open, onOpenChange }: CreateBotDialogProps) {
  const navigate = useNavigate();
  const createBot = useCreateBot();

  const [name, setName] = useState("");
  const [description, setDescription] = useState("");
  const [emoji, setEmoji] = useState("🤖");
  const [error, setError] = useState<string | null>(null);

  function resetForm() {
    setName("");
    setDescription("");
    setEmoji("🤖");
    setError(null);
  }

  function handleOpenChange(nextOpen: boolean) {
    if (!nextOpen) {
      resetForm();
    }
    onOpenChange(nextOpen);
  }

  async function handleSubmit(e: React.FormEvent) {
    e.preventDefault();
    setError(null);

    const trimmedName = name.trim();
    if (trimmedName.length < 3) {
      setError("Name must be at least 3 characters.");
      return;
    }
    if (trimmedName.length > 50) {
      setError("Name must be at most 50 characters.");
      return;
    }

    createBot.mutate(
      {
        name: trimmedName,
        description: description.trim() || undefined,
        emoji: emoji || undefined,
      },
      {
        onSuccess: (bot) => {
          handleOpenChange(false);
          navigate({ to: "/bots/$botId", params: { botId: bot.id } });
        },
        onError: (err) => {
          setError(err.message || "Failed to create bot.");
        },
      },
    );
  }

  return (
    <Dialog open={open} onOpenChange={handleOpenChange}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>Create a new bot</DialogTitle>
          <DialogDescription>
            Give your bot a name and personality. You can customize its soul
            later.
          </DialogDescription>
        </DialogHeader>

        <form onSubmit={handleSubmit} className="space-y-4">
          {/* Emoji picker */}
          <div className="space-y-2">
            <label className="text-sm font-medium">Emoji</label>
            <div className="flex flex-wrap gap-1.5">
              {EMOJI_OPTIONS.map((e) => (
                <button
                  key={e}
                  type="button"
                  onClick={() => setEmoji(e)}
                  className={`flex size-9 items-center justify-center rounded-md border text-lg transition-colors ${
                    emoji === e
                      ? "border-primary bg-accent"
                      : "border-transparent hover:bg-accent/50"
                  }`}
                >
                  {e}
                </button>
              ))}
            </div>
          </div>

          {/* Name */}
          <div className="space-y-2">
            <label htmlFor="bot-name" className="text-sm font-medium">
              Name <span className="text-destructive">*</span>
            </label>
            <Input
              id="bot-name"
              placeholder="My Assistant"
              value={name}
              onChange={(e) => setName(e.target.value)}
              maxLength={50}
              autoFocus
            />
          </div>

          {/* Description */}
          <div className="space-y-2">
            <label htmlFor="bot-description" className="text-sm font-medium">
              Description
            </label>
            <textarea
              id="bot-description"
              placeholder="A helpful bot for..."
              value={description}
              onChange={(e) => setDescription(e.target.value)}
              rows={3}
              className="w-full rounded-md border bg-transparent px-3 py-2 text-sm shadow-xs placeholder:text-muted-foreground focus-visible:border-ring focus-visible:ring-ring/50 focus-visible:ring-[3px] outline-none dark:bg-input/30 dark:border-input"
            />
          </div>

          {/* Error */}
          {error && (
            <p className="text-sm text-destructive">{error}</p>
          )}

          <DialogFooter>
            <Button
              type="button"
              variant="outline"
              onClick={() => handleOpenChange(false)}
            >
              Cancel
            </Button>
            <Button type="submit" disabled={createBot.isPending}>
              {createBot.isPending && (
                <Loader2 className="size-4 animate-spin" />
              )}
              Create Bot
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}
