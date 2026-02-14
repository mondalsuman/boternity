/**
 * Visual workflow builder page.
 *
 * Full-viewport React Flow canvas that loads a workflow definition,
 * converts it to nodes/edges, and allows visual editing with save.
 */

import { useCallback, useState } from "react";
import { createFileRoute, Link } from "@tanstack/react-router";
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { ArrowLeft, Save, Loader2 } from "lucide-react";
import { toast } from "sonner";
import type { Node, Edge } from "@xyflow/react";

import { fetchWorkflow, updateWorkflow } from "@/lib/api/workflows";
import {
  WorkflowCanvas,
  definitionToFlow,
  flowToDefinition,
} from "@/components/workflow/WorkflowCanvas";
import type { WorkflowDefinition } from "@/types/workflow";
import { Button } from "@/components/ui/button";
import { Skeleton } from "@/components/ui/skeleton";

export const Route = createFileRoute("/workflows/builder/$workflowId")({
  component: WorkflowBuilderPage,
});

function WorkflowBuilderPage() {
  const { workflowId } = Route.useParams();
  const queryClient = useQueryClient();

  // Track current canvas state for save
  const [canvasNodes, setCanvasNodes] = useState<Node[]>([]);
  const [canvasEdges, setCanvasEdges] = useState<Edge[]>([]);
  const [hasChanges, setHasChanges] = useState(false);

  const {
    data: workflow,
    isLoading,
    error,
  } = useQuery({
    queryKey: ["workflow", workflowId],
    queryFn: () => fetchWorkflow(workflowId),
  });

  const saveMutation = useMutation({
    mutationFn: (def: WorkflowDefinition) =>
      updateWorkflow(workflowId, def),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["workflow", workflowId] });
      setHasChanges(false);
      toast.success("Workflow saved");
    },
    onError: (err: Error) => {
      toast.error(`Failed to save: ${err.message}`);
    },
  });

  const handleCanvasChange = useCallback(
    (nodes: Node[], edges: Edge[]) => {
      setCanvasNodes(nodes);
      setCanvasEdges(edges);
      setHasChanges(true);
    },
    [],
  );

  const handleSave = useCallback(() => {
    if (!workflow) return;
    const updated = flowToDefinition(workflow, canvasNodes, canvasEdges);
    saveMutation.mutate(updated);
  }, [workflow, canvasNodes, canvasEdges, saveMutation]);

  // Loading state
  if (isLoading) {
    return (
      <div className="h-[calc(100vh-3rem)] flex flex-col">
        <div className="flex items-center gap-2 p-3 border-b shrink-0">
          <Skeleton className="h-8 w-20" />
          <Skeleton className="h-4 w-48" />
        </div>
        <div className="flex-1 flex items-center justify-center">
          <Loader2 className="size-8 animate-spin text-muted-foreground" />
        </div>
      </div>
    );
  }

  // Error state
  if (error || !workflow) {
    return (
      <div className="h-[calc(100vh-3rem)] flex flex-col">
        <div className="flex items-center gap-2 p-3 border-b shrink-0">
          <Button variant="ghost" size="sm" asChild>
            <Link to="/workflows">
              <ArrowLeft className="size-4" />
              Back
            </Link>
          </Button>
        </div>
        <div className="flex-1 flex items-center justify-center text-muted-foreground">
          {error
            ? `Failed to load workflow: ${(error as Error).message}`
            : "Workflow not found"}
        </div>
      </div>
    );
  }

  // Convert definition to React Flow format
  const { nodes: initialNodes, edges: initialEdges } =
    definitionToFlow(workflow);

  return (
    <div className="h-[calc(100vh-3rem)] flex flex-col">
      {/* Toolbar */}
      <div className="flex items-center gap-2 p-3 border-b shrink-0 bg-card z-10">
        <Button variant="ghost" size="sm" asChild>
          <Link
            to="/workflows/$workflowId"
            params={{ workflowId }}
          >
            <ArrowLeft className="size-4" />
            Back
          </Link>
        </Button>

        <div className="flex-1 min-w-0">
          <h2 className="text-sm font-medium truncate">{workflow.name}</h2>
          <p className="text-xs text-muted-foreground">
            {workflow.steps.length} steps &middot; v{workflow.version}
          </p>
        </div>

        <Button
          variant="default"
          size="sm"
          onClick={handleSave}
          disabled={!hasChanges || saveMutation.isPending}
        >
          {saveMutation.isPending ? (
            <Loader2 className="size-4 animate-spin" />
          ) : (
            <Save className="size-4" />
          )}
          Save
        </Button>
      </div>

      {/* Canvas */}
      <div className="flex-1">
        <WorkflowCanvas
          initialNodes={initialNodes}
          initialEdges={initialEdges}
          onChange={handleCanvasChange}
        />
      </div>
    </div>
  );
}
