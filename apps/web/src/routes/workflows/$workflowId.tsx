/**
 * Workflow detail page with run history and definition tabs.
 *
 * Shows workflow metadata, trigger badges, and tabbed content for
 * runs (with expandable step logs), definition YAML view, and
 * a link to the visual builder.
 */

import { useState } from "react";
import { createFileRoute, Link } from "@tanstack/react-router";
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import {
  Play,
  Trash2,
  ArrowLeft,
  Clock,
  Webhook,
  Hand,
  Zap,
  Eye,
  ChevronDown,
  ChevronRight,
  CheckCircle2,
  XCircle,
  Loader2,
  PauseCircle,
  SkipForward,
  CircleDot,
  Pencil,
} from "lucide-react";
import { toast } from "sonner";
import {
  fetchWorkflow,
  fetchRuns,
  fetchRunDetail,
  triggerWorkflow,
  deleteWorkflow,
  approveRun,
  cancelRun,
} from "@/lib/api/workflows";
import type {
  WorkflowRun,
  WorkflowStepLog,
  WorkflowRunStatus,
  WorkflowStepStatus,
  TriggerConfig,
} from "@/types/workflow";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Skeleton } from "@/components/ui/skeleton";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
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

export const Route = createFileRoute("/workflows/$workflowId")({
  component: WorkflowDetailPage,
});

/** Status badge color mapping for runs. */
function runStatusClass(status: WorkflowRunStatus): string {
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
    default:
      return "bg-muted text-muted-foreground";
  }
}

/** Icon for step status. */
function StepStatusIcon({ status }: { status: WorkflowStepStatus }) {
  switch (status) {
    case "completed":
      return <CheckCircle2 className="size-4 text-green-500" />;
    case "running":
      return <Loader2 className="size-4 text-blue-500 animate-spin" />;
    case "failed":
      return <XCircle className="size-4 text-red-500" />;
    case "skipped":
      return <SkipForward className="size-4 text-muted-foreground" />;
    case "waiting_approval":
      return <PauseCircle className="size-4 text-yellow-500" />;
    case "pending":
      return <CircleDot className="size-4 text-muted-foreground" />;
  }
}

/** Trigger badge with icon. */
function TriggerBadge({ trigger }: { trigger: TriggerConfig }) {
  const icons: Record<string, React.ReactNode> = {
    cron: <Clock className="size-3" />,
    webhook: <Webhook className="size-3" />,
    manual: <Hand className="size-3" />,
    event: <Zap className="size-3" />,
    file_watch: <Eye className="size-3" />,
  };

  const labels: Record<string, string> = {
    cron: trigger.type === "cron" ? trigger.schedule : "cron",
    webhook: trigger.type === "webhook" ? trigger.path : "webhook",
    manual: "Manual",
    event:
      trigger.type === "event" ? `${trigger.source}:${trigger.event_type}` : "event",
    file_watch: "File Watch",
  };

  return (
    <Badge variant="outline" className="gap-1 text-xs">
      {icons[trigger.type]}
      {labels[trigger.type]}
    </Badge>
  );
}

/** Duration string from two ISO dates. */
function duration(start: string, end?: string): string {
  if (!end) return "running...";
  const ms = new Date(end).getTime() - new Date(start).getTime();
  if (ms < 1000) return `${ms}ms`;
  if (ms < 60_000) return `${(ms / 1000).toFixed(1)}s`;
  return `${Math.floor(ms / 60_000)}m ${Math.floor((ms % 60_000) / 1000)}s`;
}

function WorkflowDetailPage() {
  const { workflowId } = Route.useParams();
  const queryClient = useQueryClient();

  const { data: workflow, isLoading: workflowLoading } = useQuery({
    queryKey: ["workflow", workflowId],
    queryFn: () => fetchWorkflow(workflowId),
  });

  const { data: runs, isLoading: runsLoading } = useQuery({
    queryKey: ["workflow-runs", workflowId],
    queryFn: () => fetchRuns(workflowId, 20),
  });

  const triggerMutation = useMutation({
    mutationFn: () => triggerWorkflow(workflowId),
    onSuccess: (data) => {
      queryClient.invalidateQueries({ queryKey: ["workflow-runs", workflowId] });
      toast.success(`Triggered (run: ${data.run_id.slice(0, 8)}...)`);
    },
    onError: (err: Error) => toast.error(err.message),
  });

  const deleteMutation = useMutation({
    mutationFn: () => deleteWorkflow(workflowId),
    onSuccess: () => {
      toast.success("Workflow deleted");
      // Navigate handled by user
    },
    onError: (err: Error) => toast.error(err.message),
  });

  if (workflowLoading) {
    return (
      <div className="p-4 md:p-6 space-y-4">
        <Skeleton className="h-8 w-64" />
        <Skeleton className="h-4 w-96" />
        <Skeleton className="h-64 w-full" />
      </div>
    );
  }

  if (!workflow) {
    return (
      <div className="p-4 md:p-6">
        <p className="text-muted-foreground">Workflow not found.</p>
      </div>
    );
  }

  const ownerLabel =
    workflow.owner.type === "bot" ? workflow.owner.slug : "Global";

  return (
    <div className="p-4 md:p-6 space-y-4 md:space-y-6">
      {/* Back link + header */}
      <div>
        <Link
          to="/workflows"
          className="text-sm text-muted-foreground hover:text-foreground inline-flex items-center gap-1 mb-2"
        >
          <ArrowLeft className="size-3" />
          Back to workflows
        </Link>

        <div className="flex items-start justify-between gap-4">
          <div>
            <h1 className="text-2xl font-bold tracking-tight">
              {workflow.name}
            </h1>
            {workflow.description && (
              <p className="text-muted-foreground mt-1">
                {workflow.description}
              </p>
            )}
            <div className="flex items-center gap-2 mt-2">
              <Badge variant="outline">{ownerLabel}</Badge>
              <span className="text-sm text-muted-foreground">
                v{workflow.version}
              </span>
            </div>
          </div>

          <div className="flex items-center gap-2 shrink-0">
            <Button
              variant="outline"
              size="sm"
              onClick={() => triggerMutation.mutate()}
              disabled={triggerMutation.isPending}
            >
              <Play className="size-4" />
              Trigger Now
            </Button>
            <Button variant="outline" size="sm" asChild>
              <Link
                to="/workflows/builder/$workflowId"
                params={{ workflowId }}
              >
                <Pencil className="size-4" />
                Builder
              </Link>
            </Button>
            <AlertDialog>
              <AlertDialogTrigger asChild>
                <Button variant="outline" size="sm">
                  <Trash2 className="size-4 text-destructive" />
                  Delete
                </Button>
              </AlertDialogTrigger>
              <AlertDialogContent>
                <AlertDialogHeader>
                  <AlertDialogTitle>Delete workflow?</AlertDialogTitle>
                  <AlertDialogDescription>
                    This will permanently delete "{workflow.name}" and all run
                    history.
                  </AlertDialogDescription>
                </AlertDialogHeader>
                <AlertDialogFooter>
                  <AlertDialogCancel>Cancel</AlertDialogCancel>
                  <AlertDialogAction
                    onClick={() => deleteMutation.mutate()}
                    className="bg-destructive text-destructive-foreground hover:bg-destructive/90"
                  >
                    Delete
                  </AlertDialogAction>
                </AlertDialogFooter>
              </AlertDialogContent>
            </AlertDialog>
          </div>
        </div>
      </div>

      {/* Trigger badges */}
      {workflow.triggers.length > 0 && (
        <div className="flex flex-wrap gap-2">
          {workflow.triggers.map((trigger, i) => (
            <TriggerBadge key={i} trigger={trigger} />
          ))}
        </div>
      )}

      {/* Tabs */}
      <Tabs defaultValue="runs" className="w-full">
        <TabsList>
          <TabsTrigger value="runs">Runs</TabsTrigger>
          <TabsTrigger value="definition">Definition</TabsTrigger>
          <TabsTrigger value="builder">Builder</TabsTrigger>
        </TabsList>

        <TabsContent value="runs" className="mt-4 space-y-3">
          {runsLoading ? (
            <div className="space-y-2">
              {Array.from({ length: 3 }).map((_, i) => (
                <Skeleton key={i} className="h-14 w-full" />
              ))}
            </div>
          ) : runs && runs.length > 0 ? (
            runs.map((run) => <RunRow key={run.id} run={run} />)
          ) : (
            <Card>
              <CardContent className="py-8 text-center text-muted-foreground">
                No runs yet. Trigger this workflow to see execution history.
              </CardContent>
            </Card>
          )}
        </TabsContent>

        <TabsContent value="definition" className="mt-4">
          <Card>
            <CardContent className="pt-4">
              <pre className="text-xs overflow-auto max-h-[60vh] font-mono bg-muted/50 rounded-md p-4">
                {JSON.stringify(workflow, null, 2)}
              </pre>
            </CardContent>
          </Card>
        </TabsContent>

        <TabsContent value="builder" className="mt-4">
          <Card>
            <CardContent className="flex flex-col items-center py-12">
              <Pencil className="size-12 text-muted-foreground mb-4" />
              <h3 className="text-lg font-semibold">Visual Builder</h3>
              <p className="text-sm text-muted-foreground mt-1 mb-4">
                Open the drag-and-drop workflow canvas editor.
              </p>
              <Button asChild>
                <Link
                  to="/workflows/builder/$workflowId"
                  params={{ workflowId }}
                >
                  Open Builder
                </Link>
              </Button>
            </CardContent>
          </Card>
        </TabsContent>
      </Tabs>
    </div>
  );
}

/** Expandable run row with step detail. */
function RunRow({ run }: { run: WorkflowRun }) {
  const [expanded, setExpanded] = useState(false);
  const queryClient = useQueryClient();

  const { data: detail, isLoading: detailLoading } = useQuery({
    queryKey: ["run-detail", run.id],
    queryFn: () => fetchRunDetail(run.id),
    enabled: expanded,
  });

  const approveMutation = useMutation({
    mutationFn: () => approveRun(run.id),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["run-detail", run.id] });
      queryClient.invalidateQueries({ queryKey: ["workflow-runs"] });
      toast.success("Run approved");
    },
    onError: (err: Error) => toast.error(err.message),
  });

  const cancelMutation = useMutation({
    mutationFn: () => cancelRun(run.id),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["run-detail", run.id] });
      queryClient.invalidateQueries({ queryKey: ["workflow-runs"] });
      toast.success("Run cancelled");
    },
    onError: (err: Error) => toast.error(err.message),
  });

  return (
    <Card>
      <button
        className="w-full text-left"
        onClick={() => setExpanded(!expanded)}
      >
        <CardHeader className="flex flex-row items-center gap-3 py-3">
          {expanded ? (
            <ChevronDown className="size-4 shrink-0 text-muted-foreground" />
          ) : (
            <ChevronRight className="size-4 shrink-0 text-muted-foreground" />
          )}
          <div className="min-w-0 flex-1 flex items-center gap-3">
            <Badge
              variant="outline"
              className={`shrink-0 ${runStatusClass(run.status)}`}
            >
              {run.status}
            </Badge>
            <span className="text-sm truncate">
              {run.id.slice(0, 8)}...
            </span>
            <Badge variant="secondary" className="text-xs shrink-0">
              {run.trigger_type}
            </Badge>
            <span className="text-xs text-muted-foreground ml-auto shrink-0">
              {duration(run.started_at, run.completed_at)}
            </span>
            <span className="text-xs text-muted-foreground shrink-0">
              {new Date(run.started_at).toLocaleString()}
            </span>
          </div>
        </CardHeader>
      </button>

      {expanded && (
        <CardContent className="pt-0 pb-4">
          {/* Run actions */}
          {(run.status === "running" || run.status === "paused") && (
            <div className="flex gap-2 mb-3">
              {run.status === "paused" && (
                <Button
                  variant="outline"
                  size="sm"
                  onClick={() => approveMutation.mutate()}
                  disabled={approveMutation.isPending}
                >
                  Approve
                </Button>
              )}
              <Button
                variant="outline"
                size="sm"
                onClick={() => cancelMutation.mutate()}
                disabled={cancelMutation.isPending}
              >
                Cancel Run
              </Button>
            </div>
          )}

          {/* Error */}
          {run.error && (
            <div className="bg-red-500/10 border border-red-500/20 rounded-md p-3 mb-3 text-sm text-red-500">
              {run.error}
            </div>
          )}

          {/* Step logs */}
          {detailLoading ? (
            <div className="space-y-2">
              <Skeleton className="h-10 w-full" />
              <Skeleton className="h-10 w-full" />
            </div>
          ) : detail?.steps && detail.steps.length > 0 ? (
            <div className="space-y-2">
              {detail.steps.map((step) => (
                <StepLogRow key={step.id} step={step} />
              ))}
            </div>
          ) : (
            <p className="text-sm text-muted-foreground">
              No step data available.
            </p>
          )}
        </CardContent>
      )}
    </Card>
  );
}

/** Individual step log row. */
function StepLogRow({ step }: { step: WorkflowStepLog }) {
  const [expanded, setExpanded] = useState(false);

  return (
    <div className="border rounded-md">
      <button
        className="w-full text-left flex items-center gap-2 px-3 py-2"
        onClick={() => setExpanded(!expanded)}
      >
        <StepStatusIcon status={step.status} />
        <span className="text-sm font-medium flex-1">{step.step_name}</span>
        <Badge variant="secondary" className="text-xs">
          attempt {step.attempt}
        </Badge>
        {step.started_at && step.completed_at && (
          <span className="text-xs text-muted-foreground">
            {duration(step.started_at, step.completed_at)}
          </span>
        )}
        {expanded ? (
          <ChevronDown className="size-3 text-muted-foreground" />
        ) : (
          <ChevronRight className="size-3 text-muted-foreground" />
        )}
      </button>

      {expanded && (
        <div className="px-3 pb-3 space-y-2">
          {step.error && (
            <div className="bg-red-500/10 border border-red-500/20 rounded p-2 text-xs text-red-500 font-mono">
              {step.error}
            </div>
          )}
          {step.output != null && (
            <div>
              <p className="text-xs font-medium text-muted-foreground mb-1">
                Output
              </p>
              <pre className="text-xs bg-muted/50 rounded p-2 overflow-auto max-h-40 font-mono">
                {typeof step.output === "string"
                  ? step.output
                  : JSON.stringify(step.output, null, 2)}
              </pre>
            </div>
          )}
          {step.input != null && (
            <div>
              <p className="text-xs font-medium text-muted-foreground mb-1">
                Input
              </p>
              <pre className="text-xs bg-muted/50 rounded p-2 overflow-auto max-h-40 font-mono">
                {typeof step.input === "string"
                  ? step.input
                  : JSON.stringify(step.input, null, 2)}
              </pre>
            </div>
          )}
        </div>
      )}
    </div>
  );
}
