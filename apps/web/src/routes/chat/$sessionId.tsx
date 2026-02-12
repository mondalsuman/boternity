import { createFileRoute } from "@tanstack/react-router";
import { Skeleton } from "@/components/ui/skeleton";

export const Route = createFileRoute("/chat/$sessionId")({
  component: ChatSessionPage,
});

function ChatSessionPage() {
  const { sessionId } = Route.useParams();

  return (
    <div className="flex flex-col h-[calc(100vh-3rem)]">
      {/* Chat header */}
      <div className="flex items-center gap-3 border-b px-4 py-3">
        <Skeleton className="h-8 w-8 rounded-full" />
        <div>
          <Skeleton className="h-5 w-32" />
          <Skeleton className="h-3 w-20 mt-1" />
        </div>
      </div>

      {/* Messages area */}
      <div className="flex-1 overflow-auto p-4 space-y-4">
        <p className="text-center text-muted-foreground text-sm">
          Session: {sessionId.slice(0, 8)}...
        </p>
        <Skeleton className="h-16 w-3/4" />
        <Skeleton className="h-16 w-2/3 ml-auto" />
        <Skeleton className="h-16 w-3/4" />
      </div>

      {/* Input area */}
      <div className="border-t p-4">
        <Skeleton className="h-12 w-full rounded-lg" />
      </div>
    </div>
  );
}
