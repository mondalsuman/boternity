import { Bot, Sparkles } from "lucide-react";
import { Button } from "@/components/ui/button";

interface EmptyStateProps {
  onCreateBot: () => void;
}

export function EmptyState({ onCreateBot }: EmptyStateProps) {
  return (
    <div className="flex flex-col items-center justify-center gap-6 py-24 text-center">
      {/* Illustration composed from Lucide icons */}
      <div className="relative">
        <div className="flex size-20 items-center justify-center rounded-2xl bg-muted">
          <Bot className="size-10 text-muted-foreground" />
        </div>
        <div className="absolute -right-2 -top-2 flex size-8 items-center justify-center rounded-full bg-primary text-primary-foreground">
          <Sparkles className="size-4" />
        </div>
      </div>

      <div className="space-y-2">
        <h2 className="text-xl font-semibold tracking-tight">
          Create your first bot
        </h2>
        <p className="text-sm text-muted-foreground max-w-sm">
          Get started by creating a bot with a unique personality. Give it a
          name, a soul, and start chatting.
        </p>
      </div>

      <Button size="lg" onClick={onCreateBot}>
        Create Bot
      </Button>
    </div>
  );
}
