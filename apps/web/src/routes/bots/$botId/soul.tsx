import { createFileRoute } from "@tanstack/react-router";
import { Skeleton } from "@/components/ui/skeleton";

export const Route = createFileRoute("/bots/$botId/soul")({
  component: BotSoulPage,
});

function BotSoulPage() {
  return (
    <div className="space-y-4">
      <p className="text-muted-foreground">
        Edit the soul, identity, and user files for this bot.
      </p>
      <div className="grid md:grid-cols-2 gap-4">
        <Skeleton className="h-[400px] rounded-lg" />
        <Skeleton className="h-[400px] rounded-lg" />
      </div>
    </div>
  );
}
