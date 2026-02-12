import { createFileRoute } from "@tanstack/react-router";
import { Skeleton } from "@/components/ui/skeleton";

export const Route = createFileRoute("/bots/$botId/chat")({
  component: BotChatPage,
});

function BotChatPage() {
  return (
    <div className="space-y-4">
      <p className="text-muted-foreground">
        Chat with this bot. Streaming responses will appear here.
      </p>
      <div className="flex flex-col gap-3">
        <Skeleton className="h-16 w-3/4" />
        <Skeleton className="h-16 w-2/3 ml-auto" />
        <Skeleton className="h-16 w-3/4" />
      </div>
      <Skeleton className="h-12 w-full" />
    </div>
  );
}
