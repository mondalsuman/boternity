/**
 * Workflow template picker dialog.
 *
 * Displays 4 built-in templates (Data Pipeline, Approval Flow, Multi-Bot,
 * Scheduled Report) as cards. Selecting one populates the builder canvas.
 */

import {
  Database,
  ShieldCheck,
  Bot,
  Calendar,
} from "lucide-react";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogDescription,
} from "@/components/ui/dialog";
import { Badge } from "@/components/ui/badge";
import type { StepDefinition, TriggerConfig } from "@/types/workflow";

// ---------------------------------------------------------------------------
// Template definitions
// ---------------------------------------------------------------------------

interface WorkflowTemplate {
  id: string;
  name: string;
  description: string;
  icon: React.ComponentType<{ className?: string }>;
  tags: string[];
  triggers: TriggerConfig[];
  steps: StepDefinition[];
}

const TEMPLATES: WorkflowTemplate[] = [
  {
    id: "data-pipeline",
    name: "Data Pipeline",
    description:
      "Fetch data from an API, transform it with code, then store the results via a skill.",
    icon: Database,
    tags: ["http", "code", "skill"],
    triggers: [{ type: "manual" }],
    steps: [
      {
        id: "fetch-data",
        name: "Fetch Data",
        type: "http",
        depends_on: [],
        config: { type: "http", method: "GET", url: "https://api.example.com/data" },
      },
      {
        id: "transform",
        name: "Transform",
        type: "code",
        depends_on: ["fetch-data"],
        config: {
          type: "code",
          language: "type_script",
          source:
            "// Transform the fetched data\nconst data = input;\nreturn data.map(item => ({ ...item, processed: true }));",
        },
      },
      {
        id: "store",
        name: "Store Results",
        type: "skill",
        depends_on: ["transform"],
        config: { type: "skill", skill: "data-store", input: "{{ steps.transform.output }}" },
      },
    ],
  },
  {
    id: "approval-flow",
    name: "Approval Flow",
    description:
      "Agent generates content, waits for human approval, then publishes or revises.",
    icon: ShieldCheck,
    tags: ["agent", "approval", "conditional"],
    triggers: [{ type: "manual" }],
    steps: [
      {
        id: "generate",
        name: "Generate Content",
        type: "agent",
        depends_on: [],
        config: { type: "agent", bot: "writer-bot", prompt: "Generate a blog post about {{ input.topic }}" },
      },
      {
        id: "review",
        name: "Human Review",
        type: "approval",
        depends_on: ["generate"],
        config: { type: "approval", prompt: "Review the generated content and approve or reject.", timeout_secs: 86400 },
      },
      {
        id: "check-result",
        name: "Check Approval",
        type: "conditional",
        depends_on: ["review"],
        config: {
          type: "conditional",
          condition: "{{ steps.review.output.approved }}",
          then_steps: ["publish"],
          else_steps: ["revise"],
        },
      },
      {
        id: "publish",
        name: "Publish",
        type: "skill",
        depends_on: [],
        config: { type: "skill", skill: "publisher", input: "{{ steps.generate.output }}" },
      },
      {
        id: "revise",
        name: "Revise Content",
        type: "agent",
        depends_on: [],
        config: {
          type: "agent",
          bot: "writer-bot",
          prompt: "Revise the content based on feedback: {{ steps.review.output.feedback }}",
        },
      },
    ],
  },
  {
    id: "multi-bot",
    name: "Multi-Bot Collaboration",
    description:
      "Multiple agents work together: researcher gathers info, analyst processes, reporter summarizes.",
    icon: Bot,
    tags: ["agent", "multi-bot"],
    triggers: [{ type: "manual" }],
    steps: [
      {
        id: "research",
        name: "Research",
        type: "agent",
        depends_on: [],
        config: { type: "agent", bot: "researcher", prompt: "Research the topic: {{ input.topic }}" },
      },
      {
        id: "analyze",
        name: "Analyze",
        type: "agent",
        depends_on: ["research"],
        config: {
          type: "agent",
          bot: "analyst",
          prompt: "Analyze the research findings: {{ steps.research.output }}",
        },
      },
      {
        id: "report",
        name: "Create Report",
        type: "agent",
        depends_on: ["analyze"],
        config: {
          type: "agent",
          bot: "reporter",
          prompt: "Create a summary report from analysis: {{ steps.analyze.output }}",
        },
      },
    ],
  },
  {
    id: "scheduled-report",
    name: "Scheduled Report",
    description:
      "Runs on a daily cron schedule: fetches metrics, generates a summary, and sends notifications.",
    icon: Calendar,
    tags: ["cron", "http", "agent"],
    triggers: [{ type: "cron", schedule: "0 9 * * *", timezone: "UTC" }],
    steps: [
      {
        id: "fetch-metrics",
        name: "Fetch Metrics",
        type: "http",
        depends_on: [],
        config: { type: "http", method: "GET", url: "https://api.example.com/metrics" },
      },
      {
        id: "summarize",
        name: "Summarize",
        type: "agent",
        depends_on: ["fetch-metrics"],
        config: {
          type: "agent",
          bot: "summarizer",
          prompt: "Create a daily summary of these metrics: {{ steps.fetch-metrics.output }}",
        },
      },
      {
        id: "notify",
        name: "Send Notification",
        type: "http",
        depends_on: ["summarize"],
        config: {
          type: "http",
          method: "POST",
          url: "https://hooks.example.com/notify",
          body: '{ "text": "{{ steps.summarize.output }}" }',
        },
      },
    ],
  },
];

// ---------------------------------------------------------------------------
// Template card
// ---------------------------------------------------------------------------

function TemplateCard({
  template,
  onSelect,
}: {
  template: WorkflowTemplate;
  onSelect: (template: WorkflowTemplate) => void;
}) {
  const Icon = template.icon;

  return (
    <button
      onClick={() => onSelect(template)}
      className="flex flex-col gap-3 p-4 rounded-lg border bg-card hover:bg-accent/50 text-left transition-colors group"
    >
      <div className="flex items-center gap-3">
        <div className="shrink-0 flex items-center justify-center size-10 rounded-lg bg-muted group-hover:bg-primary/10 transition-colors">
          <Icon className="size-5 text-muted-foreground group-hover:text-primary transition-colors" />
        </div>
        <div className="min-w-0">
          <h4 className="text-sm font-semibold">{template.name}</h4>
          <p className="text-xs text-muted-foreground">{template.steps.length} steps</p>
        </div>
      </div>

      <p className="text-xs text-muted-foreground leading-relaxed">
        {template.description}
      </p>

      <div className="flex flex-wrap gap-1">
        {template.tags.map((tag) => (
          <Badge key={tag} variant="secondary" className="text-[10px]">
            {tag}
          </Badge>
        ))}
      </div>
    </button>
  );
}

// ---------------------------------------------------------------------------
// WorkflowTemplates dialog
// ---------------------------------------------------------------------------

interface WorkflowTemplatesProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onSelect: (template: { triggers: TriggerConfig[]; steps: StepDefinition[] }) => void;
}

export function WorkflowTemplates({
  open,
  onOpenChange,
  onSelect,
}: WorkflowTemplatesProps) {
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-2xl">
        <DialogHeader>
          <DialogTitle>Start from a template</DialogTitle>
          <DialogDescription>
            Choose a template to pre-populate your workflow with steps and
            configuration. You can customize everything after.
          </DialogDescription>
        </DialogHeader>

        <div className="grid grid-cols-1 sm:grid-cols-2 gap-3 mt-2 max-h-[60vh] overflow-y-auto">
          {TEMPLATES.map((template) => (
            <TemplateCard
              key={template.id}
              template={template}
              onSelect={(t) => {
                onSelect({ triggers: t.triggers, steps: t.steps });
                onOpenChange(false);
              }}
            />
          ))}
        </div>
      </DialogContent>
    </Dialog>
  );
}
