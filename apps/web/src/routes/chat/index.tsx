import { createFileRoute } from "@tanstack/react-router";
import { Skeleton } from "@/components/ui/skeleton";
import { MessageCircle } from "lucide-react";

export const Route = createFileRoute("/chat/")({
  component: ChatHubPage,
});

function ChatHubPage() {
  return (
    <div className="p-6 space-y-6">
      <div>
        <h1 className="text-2xl font-bold tracking-tight">Chat</h1>
        <p className="text-muted-foreground">
          Start a conversation with any of your bots.
        </p>
      </div>

      {/* Empty state: grid of available bots */}
      <div className="flex flex-col items-center justify-center py-12 text-center">
        <MessageCircle className="h-12 w-12 text-muted-foreground mb-4" />
        <h2 className="text-lg font-semibold">No active sessions</h2>
        <p className="text-muted-foreground max-w-md mt-1">
          Select a bot from the sidebar or click below to start a new
          conversation.
        </p>
      </div>

      {/* Bot selection grid placeholder */}
      <div className="grid gap-4 md:grid-cols-2 lg:grid-cols-3">
        <Skeleton className="h-32 rounded-lg" />
        <Skeleton className="h-32 rounded-lg" />
        <Skeleton className="h-32 rounded-lg" />
      </div>
    </div>
  );
}
