/**
 * Visual workflow builder page.
 *
 * Full-viewport builder with:
 * - Left sidebar: NodePalette (draggable step types)
 * - Center: React Flow canvas OR YAML editor (toggle)
 * - Right sidebar: StepConfigPanel (when node selected)
 * - Top toolbar: Save, Canvas/YAML toggle, Auto-layout, Undo/Redo,
 *   Group/Ungroup, Templates, Test Step (placeholder), Run (placeholder)
 */

import { useCallback, useRef, useState } from "react";
import { createFileRoute, Link } from "@tanstack/react-router";
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import {
  ArrowLeft,
  Save,
  Loader2,
  LayoutGrid,
  FileCode2,
  Undo2,
  Redo2,
  Group,
  Ungroup,
  LayoutTemplate,
  Play,
  Square,
  FlaskConical,
  AlignVerticalSpaceAround,
  CheckCircle2,
  XCircle,
  Clock,
} from "lucide-react";
import { toast } from "sonner";
import type { Node, Edge } from "@xyflow/react";
import { ReactFlowProvider } from "@xyflow/react";

import { fetchWorkflow, updateWorkflow, triggerWorkflow, cancelRun } from "@/lib/api/workflows";
import { useAgentWebSocket } from "@/hooks/use-websocket";
import { useWorkflowEvents } from "@/hooks/use-workflow-events";
import {
  WorkflowCanvas,
  definitionToFlow,
  flowToDefinition,
} from "@/components/workflow/WorkflowCanvas";
import type { WorkflowCanvasHandle } from "@/components/workflow/WorkflowCanvas";
import { NodePalette } from "@/components/workflow/panels/NodePalette";
import { StepConfigPanel } from "@/components/workflow/panels/StepConfigPanel";
import { YamlEditor } from "@/components/workflow/YamlEditor";
import { WorkflowTemplates } from "@/components/workflow/WorkflowTemplates";
import type {
  WorkflowDefinition,
  StepDefinition,
  TriggerConfig,
} from "@/types/workflow";
import { Button } from "@/components/ui/button";
import { Skeleton } from "@/components/ui/skeleton";
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@/components/ui/tooltip";

export const Route = createFileRoute("/workflows/builder/$workflowId")({
  component: WorkflowBuilderPage,
});

// ---------------------------------------------------------------------------
// YAML serialization helpers (minimal, no external dep)
// ---------------------------------------------------------------------------

function workflowToYaml(def: WorkflowDefinition): string {
  const lines: string[] = [];
  lines.push(`name: "${def.name}"`);
  if (def.description) lines.push(`description: "${def.description}"`);
  lines.push(`version: "${def.version}"`);
  if (def.timeout_secs) lines.push(`timeout_secs: ${def.timeout_secs}`);
  if (def.concurrency) lines.push(`concurrency: ${def.concurrency}`);

  lines.push("triggers:");
  for (const trigger of def.triggers) {
    lines.push(`  - type: ${trigger.type}`);
    if (trigger.type === "cron") {
      lines.push(`    schedule: "${trigger.schedule}"`);
      if (trigger.timezone) lines.push(`    timezone: "${trigger.timezone}"`);
    }
  }

  lines.push("steps:");
  for (const step of def.steps) {
    lines.push(`  - id: ${step.id}`);
    lines.push(`    name: "${step.name}"`);
    lines.push(`    type: ${step.type}`);
    if (step.depends_on.length > 0) {
      lines.push(`    depends_on: [${step.depends_on.join(", ")}]`);
    }
    if (step.condition) lines.push(`    condition: "${step.condition}"`);
    if (step.timeout_secs) lines.push(`    timeout_secs: ${step.timeout_secs}`);
  }

  return lines.join("\n") + "\n";
}

// ---------------------------------------------------------------------------
// Toolbar button component
// ---------------------------------------------------------------------------

function ToolbarButton({
  icon: Icon,
  label,
  onClick,
  disabled = false,
  active = false,
  variant = "ghost",
}: {
  icon: React.ComponentType<{ className?: string }>;
  label: string;
  onClick?: () => void;
  disabled?: boolean;
  active?: boolean;
  variant?: "ghost" | "default" | "outline";
}) {
  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <Button
          variant={active ? "default" : variant}
          size="sm"
          onClick={onClick}
          disabled={disabled}
          className="gap-1.5"
        >
          <Icon className="size-4" />
          <span className="hidden lg:inline text-xs">{label}</span>
        </Button>
      </TooltipTrigger>
      <TooltipContent side="bottom">{label}</TooltipContent>
    </Tooltip>
  );
}

// ---------------------------------------------------------------------------
// Main builder page
// ---------------------------------------------------------------------------

type EditorMode = "canvas" | "yaml";

function WorkflowBuilderPage() {
  const { workflowId } = Route.useParams();
  const queryClient = useQueryClient();

  // Canvas state
  const [canvasNodes, setCanvasNodes] = useState<Node[]>([]);
  const [canvasEdges, setCanvasEdges] = useState<Edge[]>([]);
  const [hasChanges, setHasChanges] = useState(false);

  // Editor mode
  const [activeEditor, setActiveEditor] = useState<EditorMode>("canvas");
  const [yamlValue, setYamlValue] = useState("");

  // Node selection (for config panel)
  const [selectedNode, setSelectedNode] = useState<Node | null>(null);

  // Templates dialog
  const [templatesOpen, setTemplatesOpen] = useState(false);

  // Canvas imperative handle
  const canvasHandleRef = useRef<WorkflowCanvasHandle | null>(null);

  // WebSocket for live execution events
  const wsUrl = `${window.location.protocol === "https:" ? "wss:" : "ws:"}//${window.location.host}/ws/events`;
  const ws = useAgentWebSocket(wsUrl);
  const execution = useWorkflowEvents({
    onEvent: ws.onEvent,
    sendCommand: ws.sendCommand,
  });

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

  // Run workflow mutation
  const runMutation = useMutation({
    mutationFn: () => triggerWorkflow(workflowId),
    onSuccess: (data) => {
      execution.startTracking(data.run_id);
      toast.success("Workflow started");
    },
    onError: (err: Error) => {
      toast.error(`Failed to run: ${err.message}`);
    },
  });

  const handleRunWorkflow = useCallback(() => {
    if (execution.isRunning) return;
    runMutation.mutate();
  }, [runMutation, execution.isRunning]);

  const handleCancelRun = useCallback(() => {
    if (execution.runId) {
      cancelRun(execution.runId)
        .then(() => {
          execution.stopTracking();
          toast.success("Workflow cancelled");
        })
        .catch((err: Error) => {
          toast.error(`Failed to cancel: ${err.message}`);
        });
    }
  }, [execution]);

  const handleDismissExecution = useCallback(() => {
    execution.reset();
  }, [execution]);

  // Test step: dry run a single selected step
  const handleTestStep = useCallback(() => {
    if (!selectedNode) {
      toast.error("Select a step to test");
      return;
    }
    toast.info(`Test step "${selectedNode.data?.label}" - dry run not yet wired to backend`);
  }, [selectedNode]);

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

  // ---------------------------------------------------------------------------
  // Editor mode toggle with bidirectional sync
  // ---------------------------------------------------------------------------

  const handleToggleEditor = useCallback(
    (mode: EditorMode) => {
      if (mode === activeEditor) return;

      if (mode === "yaml" && workflow) {
        // Sync canvas -> YAML
        const updated = flowToDefinition(workflow, canvasNodes, canvasEdges);
        setYamlValue(workflowToYaml(updated));
      }
      // When switching back to canvas, we keep the existing canvas nodes
      // (YAML changes are informational; full parse-back is a future enhancement)

      setActiveEditor(mode);
    },
    [activeEditor, workflow, canvasNodes, canvasEdges],
  );

  const handleYamlChange = useCallback(
    (_yaml: string) => {
      // Mark as changed; full YAML -> canvas sync is a future enhancement
      setHasChanges(true);
    },
    [],
  );

  // ---------------------------------------------------------------------------
  // Node config panel
  // ---------------------------------------------------------------------------

  const handleNodeSelect = useCallback((node: Node | null) => {
    setSelectedNode(node);
  }, []);

  const handleUpdateNode = useCallback(
    (nodeId: string, data: Record<string, unknown>) => {
      setCanvasNodes((prev) =>
        prev.map((n) => (n.id === nodeId ? { ...n, data: { ...n.data, ...data } } : n)),
      );
      setHasChanges(true);
    },
    [],
  );

  const handleCloseConfig = useCallback(() => {
    setSelectedNode(null);
  }, []);

  // ---------------------------------------------------------------------------
  // Template selection
  // ---------------------------------------------------------------------------

  const handleTemplateSelect = useCallback(
    (template: { triggers: TriggerConfig[]; steps: StepDefinition[] }) => {
      if (!workflow) return;

      const tempDef: WorkflowDefinition = {
        ...workflow,
        triggers: template.triggers,
        steps: template.steps,
      };

      const { nodes, edges } = definitionToFlow(tempDef);
      setCanvasNodes(nodes);
      setCanvasEdges(edges);
      setHasChanges(true);
      toast.success("Template applied");
    },
    [workflow],
  );

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
      <div className="flex items-center gap-1.5 p-2 border-b shrink-0 bg-card z-10">
        <Button variant="ghost" size="sm" asChild>
          <Link
            to="/workflows/$workflowId"
            params={{ workflowId }}
          >
            <ArrowLeft className="size-4" />
          </Link>
        </Button>

        <div className="flex-1 min-w-0 mx-2">
          <h2 className="text-sm font-medium truncate">{workflow.name}</h2>
          <p className="text-[11px] text-muted-foreground">
            {workflow.steps.length} steps &middot; v{workflow.version}
          </p>
        </div>

        {/* Separator */}
        <div className="w-px h-6 bg-border mx-1" />

        {/* Editor toggle */}
        <ToolbarButton
          icon={LayoutGrid}
          label="Canvas"
          active={activeEditor === "canvas"}
          onClick={() => handleToggleEditor("canvas")}
        />
        <ToolbarButton
          icon={FileCode2}
          label="YAML"
          active={activeEditor === "yaml"}
          onClick={() => handleToggleEditor("yaml")}
        />

        <div className="w-px h-6 bg-border mx-1" />

        {/* Canvas actions */}
        <ToolbarButton
          icon={AlignVerticalSpaceAround}
          label="Auto Layout"
          onClick={() => canvasHandleRef.current?.autoLayout()}
          disabled={activeEditor !== "canvas"}
        />
        <ToolbarButton
          icon={Undo2}
          label="Undo"
          onClick={() => canvasHandleRef.current?.undo()}
          disabled={activeEditor !== "canvas" || !canvasHandleRef.current?.canUndo}
        />
        <ToolbarButton
          icon={Redo2}
          label="Redo"
          onClick={() => canvasHandleRef.current?.redo()}
          disabled={activeEditor !== "canvas" || !canvasHandleRef.current?.canRedo}
        />

        <div className="w-px h-6 bg-border mx-1" />

        {/* Grouping */}
        <ToolbarButton
          icon={Group}
          label="Group"
          onClick={() => canvasHandleRef.current?.groupSelected()}
          disabled={activeEditor !== "canvas"}
        />
        <ToolbarButton
          icon={Ungroup}
          label="Ungroup"
          onClick={() => canvasHandleRef.current?.ungroupSelected()}
          disabled={activeEditor !== "canvas"}
        />

        <div className="w-px h-6 bg-border mx-1" />

        {/* Templates */}
        <ToolbarButton
          icon={LayoutTemplate}
          label="Templates"
          onClick={() => setTemplatesOpen(true)}
        />

        <div className="w-px h-6 bg-border mx-1" />

        {/* Execution actions */}
        <ToolbarButton
          icon={FlaskConical}
          label="Test Step"
          onClick={handleTestStep}
          disabled={!selectedNode || execution.isRunning}
        />
        {execution.isRunning ? (
          <ToolbarButton
            icon={Square}
            label="Cancel"
            onClick={handleCancelRun}
            variant="outline"
          />
        ) : (
          <ToolbarButton
            icon={Play}
            label="Run"
            onClick={handleRunWorkflow}
            disabled={runMutation.isPending}
          />
        )}

        <div className="w-px h-6 bg-border mx-1" />

        {/* Save */}
        <Button
          variant="default"
          size="sm"
          onClick={handleSave}
          disabled={!hasChanges || saveMutation.isPending}
          className="gap-1.5"
        >
          {saveMutation.isPending ? (
            <Loader2 className="size-4 animate-spin" />
          ) : (
            <Save className="size-4" />
          )}
          Save
        </Button>
      </div>

      {/* Execution status bar */}
      {execution.runStatus && (
        <ExecutionStatusBar
          runStatus={execution.runStatus}
          runId={execution.runId}
          stepStatuses={execution.stepStatuses}
          totalDuration={execution.totalDuration}
          totalStepsCompleted={execution.totalStepsCompleted}
          runError={execution.runError}
          isRunning={execution.isRunning}
          onDismiss={handleDismissExecution}
        />
      )}

      {/* Main content area */}
      <div className="flex-1 flex overflow-hidden">
        {/* Left sidebar: Node palette (canvas mode only) */}
        {activeEditor === "canvas" && <NodePalette />}

        {/* Center: Canvas or YAML editor */}
        <div className="flex-1 min-w-0">
          {activeEditor === "canvas" ? (
            <ReactFlowProvider>
              <WorkflowCanvas
                initialNodes={initialNodes}
                initialEdges={initialEdges}
                onChange={handleCanvasChange}
                onNodeSelect={handleNodeSelect}
                canvasRef={canvasHandleRef}
                stepStatuses={execution.stepStatuses}
              />
            </ReactFlowProvider>
          ) : (
            <YamlEditor
              value={yamlValue}
              onChange={handleYamlChange}
            />
          )}
        </div>

        {/* Right sidebar: Step config panel (canvas mode + node selected) */}
        {activeEditor === "canvas" && selectedNode && (
          <StepConfigPanel
            selectedNode={selectedNode}
            onUpdateNode={handleUpdateNode}
            onClose={handleCloseConfig}
          />
        )}
      </div>

      {/* Templates dialog */}
      <WorkflowTemplates
        open={templatesOpen}
        onOpenChange={setTemplatesOpen}
        onSelect={handleTemplateSelect}
      />
    </div>
  );
}

// ---------------------------------------------------------------------------
// Execution status bar component
// ---------------------------------------------------------------------------

function ExecutionStatusBar({
  runStatus,
  runId,
  stepStatuses,
  totalDuration,
  totalStepsCompleted,
  runError,
  isRunning,
  onDismiss,
}: {
  runStatus: string;
  runId: string | null;
  stepStatuses: Map<string, { status: string }>;
  totalDuration: number | null;
  totalStepsCompleted: number | null;
  runError: string | null;
  isRunning: boolean;
  onDismiss: () => void;
}) {
  const completedCount = [...stepStatuses.values()].filter(
    (s) => s.status === "completed",
  ).length;
  const failedCount = [...stepStatuses.values()].filter(
    (s) => s.status === "failed",
  ).length;
  const runningCount = [...stepStatuses.values()].filter(
    (s) => s.status === "running",
  ).length;

  const bgClass =
    runStatus === "completed"
      ? "bg-green-500/10 border-green-500/30"
      : runStatus === "failed"
        ? "bg-red-500/10 border-red-500/30"
        : runStatus === "paused"
          ? "bg-yellow-500/10 border-yellow-500/30"
          : "bg-blue-500/10 border-blue-500/30";

  const StatusIcon =
    runStatus === "completed"
      ? CheckCircle2
      : runStatus === "failed"
        ? XCircle
        : runStatus === "paused"
          ? Clock
          : Loader2;

  const iconClass =
    runStatus === "completed"
      ? "text-green-500"
      : runStatus === "failed"
        ? "text-red-500"
        : runStatus === "paused"
          ? "text-yellow-500"
          : "text-blue-500 animate-spin";

  return (
    <div
      className={`flex items-center gap-3 px-3 py-1.5 border-b text-xs shrink-0 ${bgClass}`}
    >
      <StatusIcon className={`size-4 ${iconClass}`} />

      <span className="font-medium capitalize">{runStatus}</span>

      {isRunning && (
        <span className="text-muted-foreground">
          {runningCount > 0 && `${runningCount} running`}
          {completedCount > 0 && ` / ${completedCount} completed`}
        </span>
      )}

      {!isRunning && runStatus === "completed" && (
        <span className="text-muted-foreground">
          {totalStepsCompleted} steps in{" "}
          {totalDuration != null ? `${(totalDuration / 1000).toFixed(1)}s` : "?"}
        </span>
      )}

      {!isRunning && runStatus === "failed" && runError && (
        <span className="text-red-600 dark:text-red-400 truncate max-w-md" title={runError}>
          {runError}
        </span>
      )}

      {failedCount > 0 && (
        <span className="text-red-500">{failedCount} failed</span>
      )}

      {runId && (
        <span className="text-muted-foreground/60 ml-auto font-mono">
          {runId.slice(0, 8)}
        </span>
      )}

      {!isRunning && (
        <button
          onClick={onDismiss}
          className="ml-1 text-muted-foreground hover:text-foreground transition-colors"
        >
          Dismiss
        </button>
      )}
    </div>
  );
}
