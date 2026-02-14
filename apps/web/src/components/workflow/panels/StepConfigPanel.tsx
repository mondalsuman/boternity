/**
 * Right-side step configuration panel for the workflow builder.
 *
 * When a node is selected on the canvas, this panel slides open showing
 * a dynamic form based on the step type. Common fields (name, timeout,
 * retry, depends_on, condition) appear for all step types.
 */

import { useCallback, useEffect, useState } from "react";
import { X } from "lucide-react";
import type { Node } from "@xyflow/react";

import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Textarea } from "@/components/ui/textarea";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Badge } from "@/components/ui/badge";
import type { StepType } from "@/types/workflow";

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

interface StepConfigPanelProps {
  /** Currently selected node, or null when nothing is selected. */
  selectedNode: Node | null;
  /** Callback to update node data. */
  onUpdateNode: (nodeId: string, data: Record<string, unknown>) => void;
  /** Callback to close the panel. */
  onClose: () => void;
}

// ---------------------------------------------------------------------------
// Common fields section
// ---------------------------------------------------------------------------

function CommonFields({
  data,
  onChange,
}: {
  data: Record<string, unknown>;
  onChange: (key: string, value: unknown) => void;
}) {
  return (
    <div className="space-y-4">
      <div className="space-y-2">
        <Label htmlFor="step-name">Name</Label>
        <Input
          id="step-name"
          value={(data.label as string) ?? ""}
          onChange={(e) => onChange("label", e.target.value)}
          placeholder="Step name"
        />
      </div>

      <div className="space-y-2">
        <Label htmlFor="step-timeout">Timeout (seconds)</Label>
        <Input
          id="step-timeout"
          type="number"
          value={(data.timeout_secs as number) ?? ""}
          onChange={(e) =>
            onChange(
              "timeout_secs",
              e.target.value ? Number(e.target.value) : undefined,
            )
          }
          placeholder="No timeout"
        />
      </div>

      <div className="space-y-2">
        <Label htmlFor="step-retry">Max retry attempts</Label>
        <Input
          id="step-retry"
          type="number"
          min={0}
          max={10}
          value={(data.max_attempts as number) ?? ""}
          onChange={(e) =>
            onChange(
              "max_attempts",
              e.target.value ? Number(e.target.value) : undefined,
            )
          }
          placeholder="0 (no retry)"
        />
      </div>

      <div className="space-y-2">
        <Label htmlFor="step-depends">Depends on (comma-separated IDs)</Label>
        <Input
          id="step-depends"
          value={
            Array.isArray(data.depends_on)
              ? (data.depends_on as string[]).join(", ")
              : ""
          }
          onChange={(e) =>
            onChange(
              "depends_on",
              e.target.value
                .split(",")
                .map((s) => s.trim())
                .filter(Boolean),
            )
          }
          placeholder="step-1, step-2"
        />
      </div>

      <div className="space-y-2">
        <Label htmlFor="step-condition">Condition (expression)</Label>
        <Input
          id="step-condition"
          value={(data.condition as string) ?? ""}
          onChange={(e) => onChange("condition", e.target.value || undefined)}
          placeholder="e.g. {{ steps.prev.output.ok }}"
        />
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Type-specific config forms
// ---------------------------------------------------------------------------

function AgentConfig({
  data,
  onChange,
}: {
  data: Record<string, unknown>;
  onChange: (key: string, value: unknown) => void;
}) {
  return (
    <div className="space-y-4">
      <div className="space-y-2">
        <Label htmlFor="agent-bot">Bot slug</Label>
        <Input
          id="agent-bot"
          value={(data.bot as string) ?? ""}
          onChange={(e) => onChange("bot", e.target.value)}
          placeholder="my-bot"
        />
      </div>
      <div className="space-y-2">
        <Label htmlFor="agent-prompt">Prompt</Label>
        <Textarea
          id="agent-prompt"
          value={(data.prompt as string) ?? ""}
          onChange={(e) => onChange("prompt", e.target.value)}
          placeholder="Describe what the agent should do..."
          rows={4}
        />
      </div>
      <div className="space-y-2">
        <Label htmlFor="agent-model">Model (optional)</Label>
        <Input
          id="agent-model"
          value={(data.model as string) ?? ""}
          onChange={(e) => onChange("model", e.target.value || undefined)}
          placeholder="e.g. claude-sonnet-4-20250514"
        />
      </div>
    </div>
  );
}

function SkillConfig({
  data,
  onChange,
}: {
  data: Record<string, unknown>;
  onChange: (key: string, value: unknown) => void;
}) {
  return (
    <div className="space-y-4">
      <div className="space-y-2">
        <Label htmlFor="skill-name">Skill name</Label>
        <Input
          id="skill-name"
          value={(data.skill as string) ?? ""}
          onChange={(e) => onChange("skill", e.target.value)}
          placeholder="skill-name"
        />
      </div>
      <div className="space-y-2">
        <Label htmlFor="skill-input">Input (expression)</Label>
        <Textarea
          id="skill-input"
          value={(data.input as string) ?? ""}
          onChange={(e) => onChange("input", e.target.value || undefined)}
          placeholder="{{ steps.prev.output }}"
          rows={3}
        />
      </div>
    </div>
  );
}

function CodeConfig({
  data,
  onChange,
}: {
  data: Record<string, unknown>;
  onChange: (key: string, value: unknown) => void;
}) {
  return (
    <div className="space-y-4">
      <div className="space-y-2">
        <Label htmlFor="code-lang">Language</Label>
        <Select
          value={(data.language as string) ?? "type_script"}
          onValueChange={(v) => onChange("language", v)}
        >
          <SelectTrigger id="code-lang" size="sm">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="type_script">TypeScript</SelectItem>
            <SelectItem value="wasm">WASM</SelectItem>
          </SelectContent>
        </Select>
      </div>
      <div className="space-y-2">
        <Label htmlFor="code-source">Source code</Label>
        <Textarea
          id="code-source"
          value={(data.source as string) ?? ""}
          onChange={(e) => onChange("source", e.target.value)}
          placeholder="// Your code here"
          rows={8}
          className="font-mono text-xs"
        />
      </div>
    </div>
  );
}

function HttpConfig({
  data,
  onChange,
}: {
  data: Record<string, unknown>;
  onChange: (key: string, value: unknown) => void;
}) {
  return (
    <div className="space-y-4">
      <div className="space-y-2">
        <Label htmlFor="http-method">Method</Label>
        <Select
          value={(data.method as string) ?? "GET"}
          onValueChange={(v) => onChange("method", v)}
        >
          <SelectTrigger id="http-method" size="sm">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="GET">GET</SelectItem>
            <SelectItem value="POST">POST</SelectItem>
            <SelectItem value="PUT">PUT</SelectItem>
            <SelectItem value="PATCH">PATCH</SelectItem>
            <SelectItem value="DELETE">DELETE</SelectItem>
          </SelectContent>
        </Select>
      </div>
      <div className="space-y-2">
        <Label htmlFor="http-url">URL</Label>
        <Input
          id="http-url"
          value={(data.url as string) ?? ""}
          onChange={(e) => onChange("url", e.target.value)}
          placeholder="https://api.example.com/endpoint"
        />
      </div>
      <div className="space-y-2">
        <Label htmlFor="http-body">Body (optional)</Label>
        <Textarea
          id="http-body"
          value={(data.body as string) ?? ""}
          onChange={(e) => onChange("body", e.target.value || undefined)}
          placeholder='{ "key": "value" }'
          rows={4}
          className="font-mono text-xs"
        />
      </div>
    </div>
  );
}

function ConditionalConfig({
  data,
  onChange,
}: {
  data: Record<string, unknown>;
  onChange: (key: string, value: unknown) => void;
}) {
  return (
    <div className="space-y-4">
      <div className="space-y-2">
        <Label htmlFor="cond-expr">Condition expression</Label>
        <Input
          id="cond-expr"
          value={(data.condition as string) ?? ""}
          onChange={(e) => onChange("condition", e.target.value)}
          placeholder="{{ steps.prev.output.status == 'ok' }}"
        />
      </div>
      <div className="space-y-2">
        <Label htmlFor="cond-then">Then steps (comma-separated IDs)</Label>
        <Input
          id="cond-then"
          value={
            Array.isArray(data.then_steps)
              ? (data.then_steps as string[]).join(", ")
              : ""
          }
          onChange={(e) =>
            onChange(
              "then_steps",
              e.target.value
                .split(",")
                .map((s) => s.trim())
                .filter(Boolean),
            )
          }
          placeholder="step-a, step-b"
        />
      </div>
      <div className="space-y-2">
        <Label htmlFor="cond-else">Else steps (comma-separated IDs)</Label>
        <Input
          id="cond-else"
          value={
            Array.isArray(data.else_steps)
              ? (data.else_steps as string[]).join(", ")
              : ""
          }
          onChange={(e) =>
            onChange(
              "else_steps",
              e.target.value
                .split(",")
                .map((s) => s.trim())
                .filter(Boolean),
            )
          }
          placeholder="step-c"
        />
      </div>
    </div>
  );
}

function LoopConfig({
  data,
  onChange,
}: {
  data: Record<string, unknown>;
  onChange: (key: string, value: unknown) => void;
}) {
  return (
    <div className="space-y-4">
      <div className="space-y-2">
        <Label htmlFor="loop-cond">Loop condition</Label>
        <Input
          id="loop-cond"
          value={(data.condition as string) ?? ""}
          onChange={(e) => onChange("condition", e.target.value)}
          placeholder="{{ iteration < 5 }}"
        />
      </div>
      <div className="space-y-2">
        <Label htmlFor="loop-max">Max iterations</Label>
        <Input
          id="loop-max"
          type="number"
          min={1}
          value={(data.max_iterations as number) ?? ""}
          onChange={(e) =>
            onChange(
              "max_iterations",
              e.target.value ? Number(e.target.value) : undefined,
            )
          }
          placeholder="10"
        />
      </div>
      <div className="space-y-2">
        <Label htmlFor="loop-body">Body steps (comma-separated IDs)</Label>
        <Input
          id="loop-body"
          value={
            Array.isArray(data.body_steps)
              ? (data.body_steps as string[]).join(", ")
              : ""
          }
          onChange={(e) =>
            onChange(
              "body_steps",
              e.target.value
                .split(",")
                .map((s) => s.trim())
                .filter(Boolean),
            )
          }
          placeholder="step-a, step-b"
        />
      </div>
    </div>
  );
}

function ApprovalConfig({
  data,
  onChange,
}: {
  data: Record<string, unknown>;
  onChange: (key: string, value: unknown) => void;
}) {
  return (
    <div className="space-y-4">
      <div className="space-y-2">
        <Label htmlFor="approval-prompt">Approval prompt</Label>
        <Textarea
          id="approval-prompt"
          value={(data.prompt as string) ?? ""}
          onChange={(e) => onChange("prompt", e.target.value)}
          placeholder="Please review and approve this step..."
          rows={4}
        />
      </div>
      <div className="space-y-2">
        <Label htmlFor="approval-timeout">Timeout (seconds)</Label>
        <Input
          id="approval-timeout"
          type="number"
          value={(data.timeout_secs as number) ?? ""}
          onChange={(e) =>
            onChange(
              "timeout_secs",
              e.target.value ? Number(e.target.value) : undefined,
            )
          }
          placeholder="3600 (1 hour)"
        />
      </div>
    </div>
  );
}

function SubWorkflowConfig({
  data,
  onChange,
}: {
  data: Record<string, unknown>;
  onChange: (key: string, value: unknown) => void;
}) {
  return (
    <div className="space-y-4">
      <div className="space-y-2">
        <Label htmlFor="sub-name">Workflow name</Label>
        <Input
          id="sub-name"
          value={(data.workflow_name as string) ?? ""}
          onChange={(e) => onChange("workflow_name", e.target.value)}
          placeholder="data-pipeline"
        />
      </div>
      <div className="space-y-2">
        <Label htmlFor="sub-input">Input (JSON expression)</Label>
        <Textarea
          id="sub-input"
          value={
            typeof data.input === "string"
              ? data.input
              : data.input != null
                ? JSON.stringify(data.input, null, 2)
                : ""
          }
          onChange={(e) => onChange("input", e.target.value || undefined)}
          placeholder='{ "key": "{{ steps.prev.output }}" }'
          rows={4}
          className="font-mono text-xs"
        />
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Step type labels
// ---------------------------------------------------------------------------

const STEP_TYPE_LABELS: Record<StepType, string> = {
  agent: "Agent",
  skill: "Skill",
  code: "Code",
  http: "HTTP Request",
  conditional: "Conditional",
  loop: "Loop",
  approval: "Approval",
  sub_workflow: "Sub-Workflow",
};

// ---------------------------------------------------------------------------
// Main component
// ---------------------------------------------------------------------------

export function StepConfigPanel({
  selectedNode,
  onUpdateNode,
  onClose,
}: StepConfigPanelProps) {
  const [localData, setLocalData] = useState<Record<string, unknown>>({});

  // Sync local state when selected node changes
  useEffect(() => {
    if (selectedNode) {
      setLocalData({ ...selectedNode.data } as Record<string, unknown>);
    }
  }, [selectedNode]);

  const handleChange = useCallback(
    (key: string, value: unknown) => {
      setLocalData((prev) => {
        const updated = { ...prev, [key]: value };
        if (selectedNode) {
          onUpdateNode(selectedNode.id, updated);
        }
        return updated;
      });
    },
    [selectedNode, onUpdateNode],
  );

  if (!selectedNode) return null;

  const stepType = selectedNode.type as StepType;

  return (
    <div className="w-80 border-l bg-card flex flex-col h-full shrink-0">
      {/* Header */}
      <div className="flex items-center justify-between p-3 border-b shrink-0">
        <div className="flex items-center gap-2 min-w-0">
          <Badge variant="secondary" className="text-[10px] shrink-0">
            {STEP_TYPE_LABELS[stepType] ?? stepType}
          </Badge>
          <span className="text-sm font-medium truncate">
            {(localData.label as string) ?? "Untitled"}
          </span>
        </div>
        <Button variant="ghost" size="sm" onClick={onClose} className="shrink-0">
          <X className="size-4" />
        </Button>
      </div>

      {/* Body */}
      <ScrollArea className="flex-1 overflow-hidden">
        <div className="p-4 space-y-6">
          {/* Common fields */}
          <div>
            <h4 className="text-xs font-semibold text-muted-foreground uppercase tracking-wider mb-3">
              General
            </h4>
            <CommonFields data={localData} onChange={handleChange} />
          </div>

          {/* Type-specific config */}
          <div>
            <h4 className="text-xs font-semibold text-muted-foreground uppercase tracking-wider mb-3">
              {STEP_TYPE_LABELS[stepType]} Settings
            </h4>
            {stepType === "agent" && (
              <AgentConfig data={localData} onChange={handleChange} />
            )}
            {stepType === "skill" && (
              <SkillConfig data={localData} onChange={handleChange} />
            )}
            {stepType === "code" && (
              <CodeConfig data={localData} onChange={handleChange} />
            )}
            {stepType === "http" && (
              <HttpConfig data={localData} onChange={handleChange} />
            )}
            {stepType === "conditional" && (
              <ConditionalConfig data={localData} onChange={handleChange} />
            )}
            {stepType === "loop" && (
              <LoopConfig data={localData} onChange={handleChange} />
            )}
            {stepType === "approval" && (
              <ApprovalConfig data={localData} onChange={handleChange} />
            )}
            {stepType === "sub_workflow" && (
              <SubWorkflowConfig data={localData} onChange={handleChange} />
            )}
          </div>
        </div>
      </ScrollArea>
    </div>
  );
}
