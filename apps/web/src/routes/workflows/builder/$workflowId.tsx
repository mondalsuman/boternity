/**
 * Visual workflow builder page (stub - full implementation in Task 2).
 *
 * Loads a workflow definition and renders the React Flow canvas.
 */

import { createFileRoute, Link } from "@tanstack/react-router";
import { ArrowLeft } from "lucide-react";
import { Button } from "@/components/ui/button";

export const Route = createFileRoute("/workflows/builder/$workflowId")({
  component: WorkflowBuilderPage,
});

function WorkflowBuilderPage() {
  const { workflowId } = Route.useParams();

  return (
    <div className="h-[calc(100vh-3rem)] flex flex-col">
      <div className="flex items-center gap-2 p-3 border-b shrink-0">
        <Button variant="ghost" size="sm" asChild>
          <Link to="/workflows/$workflowId" params={{ workflowId }}>
            <ArrowLeft className="size-4" />
            Back
          </Link>
        </Button>
        <span className="text-sm text-muted-foreground">
          Builder: {workflowId.slice(0, 8)}...
        </span>
      </div>
      <div className="flex-1 flex items-center justify-center text-muted-foreground">
        React Flow canvas loading...
      </div>
    </div>
  );
}
