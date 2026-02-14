/**
 * Workflow list page.
 *
 * Displays all workflows in a table with search, trigger, and delete actions.
 * TanStack Query for data fetching, AlertDialog for destructive confirmations.
 */

import { useState } from "react";
import { createFileRoute, Link } from "@tanstack/react-router";
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import {
  Plus,
  Play,
  Trash2,
  Search,
  Pencil,
  Clock,
  Webhook,
  Hand,
  Eye,
  Zap,
} from "lucide-react";
import { toast } from "sonner";
import {
  fetchWorkflows,
  deleteWorkflow,
  triggerWorkflow,
} from "@/lib/api/workflows";
import type { WorkflowSummary, TriggerConfig, WorkflowRunStatus } from "@/types/workflow";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Skeleton } from "@/components/ui/skeleton";
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
  AlertDialogTrigger,
} from "@/components/ui/alert-dialog";

export const Route = createFileRoute("/workflows/")({
  component: WorkflowListPage,
});

/** Map run status to badge variant styling. */
function statusVariant(status?: WorkflowRunStatus): string {
  switch (status) {
    case "completed":
      return "bg-green-500/10 text-green-500 border-green-500/20";
    case "running":
      return "bg-blue-500/10 text-blue-500 border-blue-500/20";
    case "paused":
      return "bg-yellow-500/10 text-yellow-500 border-yellow-500/20";
    case "failed":
    case "crashed":
      return "bg-red-500/10 text-red-500 border-red-500/20";
    case "cancelled":
      return "bg-muted text-muted-foreground";
    case "pending":
      return "bg-muted text-muted-foreground";
    default:
      return "bg-muted text-muted-foreground";
  }
}

/** Icon for trigger type. */
function TriggerIcon({ trigger }: { trigger: TriggerConfig }) {
  switch (trigger.type) {
    case "cron":
      return <Clock className="size-3" />;
    case "webhook":
      return <Webhook className="size-3" />;
    case "manual":
      return <Hand className="size-3" />;
    case "event":
      return <Zap className="size-3" />;
    case "file_watch":
      return <Eye className="size-3" />;
  }
}

/** Owner display string. */
function ownerLabel(owner: WorkflowSummary["owner"]): string {
  return owner.type === "bot" ? owner.slug : "Global";
}

function WorkflowListPage() {
  const [search, setSearch] = useState("");
  const queryClient = useQueryClient();

  const { data: workflows, isLoading } = useQuery({
    queryKey: ["workflows"],
    queryFn: () => fetchWorkflows(),
    placeholderData: (prev) => prev,
  });

  const deleteMutation = useMutation({
    mutationFn: (id: string) => deleteWorkflow(id),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["workflows"] });
      toast.success("Workflow deleted");
    },
    onError: (err: Error) => {
      toast.error(`Failed to delete: ${err.message}`);
    },
  });

  const triggerMutation = useMutation({
    mutationFn: (id: string) => triggerWorkflow(id),
    onSuccess: (data) => {
      queryClient.invalidateQueries({ queryKey: ["workflows"] });
      toast.success(`Workflow triggered (run: ${data.run_id.slice(0, 8)}...)`);
    },
    onError: (err: Error) => {
      toast.error(`Failed to trigger: ${err.message}`);
    },
  });

  const filtered = (workflows ?? []).filter((wf) =>
    wf.name.toLowerCase().includes(search.toLowerCase()),
  );

  return (
    <div className="p-4 md:p-6 space-y-4 md:space-y-6">
      {/* Header */}
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold tracking-tight">Workflows</h1>
          <p className="text-muted-foreground">
            Manage and monitor your workflow pipelines.
          </p>
        </div>
        <Button asChild>
          <Link to="/workflows">
            <Plus className="size-4" />
            Create Workflow
          </Link>
        </Button>
      </div>

      {/* Search */}
      <div className="relative max-w-sm">
        <Search className="absolute left-3 top-1/2 -translate-y-1/2 size-4 text-muted-foreground" />
        <Input
          placeholder="Search workflows..."
          value={search}
          onChange={(e) => setSearch(e.target.value)}
          className="pl-9"
        />
      </div>

      {/* Loading skeleton */}
      {isLoading && (
        <div className="space-y-3">
          {Array.from({ length: 3 }).map((_, i) => (
            <Skeleton key={i} className="h-20 w-full rounded-lg" />
          ))}
        </div>
      )}

      {/* Empty state */}
      {!isLoading && filtered.length === 0 && (
        <Card>
          <CardContent className="flex flex-col items-center justify-center py-12">
            <Zap className="size-12 text-muted-foreground mb-4" />
            <h3 className="text-lg font-semibold">No workflows yet</h3>
            <p className="text-sm text-muted-foreground mt-1">
              {search
                ? "No workflows match your search."
                : "Create your first workflow to get started."}
            </p>
          </CardContent>
        </Card>
      )}

      {/* Workflow cards */}
      {!isLoading && filtered.length > 0 && (
        <div className="space-y-3">
          {filtered.map((wf) => (
            <WorkflowCard
              key={wf.id}
              workflow={wf}
              onTrigger={() => triggerMutation.mutate(wf.id)}
              onDelete={() => deleteMutation.mutate(wf.id)}
              isTriggerPending={triggerMutation.isPending}
            />
          ))}
        </div>
      )}
    </div>
  );
}

function WorkflowCard({
  workflow,
  onTrigger,
  onDelete,
  isTriggerPending,
}: {
  workflow: WorkflowSummary;
  onTrigger: () => void;
  onDelete: () => void;
  isTriggerPending: boolean;
}) {
  return (
    <Card className="hover:border-foreground/20 transition-colors">
      <CardHeader className="flex flex-row items-start justify-between gap-4 pb-3">
        <div className="min-w-0 flex-1">
          <div className="flex items-center gap-2">
            <Link
              to="/workflows/$workflowId"
              params={{ workflowId: workflow.id }}
              className="hover:underline"
            >
              <CardTitle className="text-base">{workflow.name}</CardTitle>
            </Link>
            <Badge variant="outline" className="text-xs shrink-0">
              {ownerLabel(workflow.owner)}
            </Badge>
            {workflow.last_run_status && (
              <Badge
                variant="outline"
                className={`text-xs ${statusVariant(workflow.last_run_status)}`}
              >
                {workflow.last_run_status}
              </Badge>
            )}
          </div>
          {workflow.description && (
            <p className="text-sm text-muted-foreground mt-1 truncate">
              {workflow.description}
            </p>
          )}
        </div>

        {/* Actions */}
        <div className="flex items-center gap-1 shrink-0">
          <Button
            variant="ghost"
            size="icon"
            onClick={onTrigger}
            disabled={isTriggerPending}
            title="Trigger workflow"
          >
            <Play className="size-4" />
          </Button>
          <Button variant="ghost" size="icon" asChild title="Open builder">
            <Link
              to="/workflows/builder/$workflowId"
              params={{ workflowId: workflow.id }}
            >
              <Pencil className="size-4" />
            </Link>
          </Button>
          <AlertDialog>
            <AlertDialogTrigger asChild>
              <Button variant="ghost" size="icon" title="Delete workflow">
                <Trash2 className="size-4 text-destructive" />
              </Button>
            </AlertDialogTrigger>
            <AlertDialogContent>
              <AlertDialogHeader>
                <AlertDialogTitle>Delete workflow?</AlertDialogTitle>
                <AlertDialogDescription>
                  This will permanently delete "{workflow.name}" and all its run
                  history. This action cannot be undone.
                </AlertDialogDescription>
              </AlertDialogHeader>
              <AlertDialogFooter>
                <AlertDialogCancel>Cancel</AlertDialogCancel>
                <AlertDialogAction
                  onClick={onDelete}
                  className="bg-destructive text-destructive-foreground hover:bg-destructive/90"
                >
                  Delete
                </AlertDialogAction>
              </AlertDialogFooter>
            </AlertDialogContent>
          </AlertDialog>
        </div>
      </CardHeader>
      <CardContent className="flex items-center gap-4 text-sm text-muted-foreground">
        <span>{workflow.step_count} steps</span>
        <span className="text-border">|</span>
        <span>{workflow.trigger_count} triggers</span>
        <span className="text-border">|</span>
        <span>v{workflow.version}</span>
        {workflow.last_run_at && (
          <>
            <span className="text-border">|</span>
            <span>
              Last run:{" "}
              {new Date(workflow.last_run_at).toLocaleDateString()}
            </span>
          </>
        )}
      </CardContent>
    </Card>
  );
}
