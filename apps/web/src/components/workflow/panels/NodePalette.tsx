/**
 * Left sidebar node palette for the workflow builder.
 *
 * Displays categorized, draggable step types that can be dropped onto the canvas.
 * Uses HTML5 drag-and-drop with dataTransfer to communicate the step type.
 */

import { type DragEvent, useCallback } from "react";
import {
  Bot,
  Puzzle,
  Code2,
  Globe,
  GitBranch,
  Repeat,
  ShieldCheck,
  Workflow,
} from "lucide-react";
import { ScrollArea } from "@/components/ui/scroll-area";
import type { StepType } from "@/types/workflow";

// ---------------------------------------------------------------------------
// Palette item definition
// ---------------------------------------------------------------------------

interface PaletteItem {
  type: StepType;
  label: string;
  description: string;
  icon: React.ComponentType<{ className?: string }>;
}

interface PaletteCategory {
  name: string;
  items: PaletteItem[];
}

const PALETTE_CATEGORIES: PaletteCategory[] = [
  {
    name: "AI",
    items: [
      {
        type: "agent",
        label: "Agent",
        description: "Run a bot with a prompt",
        icon: Bot,
      },
    ],
  },
  {
    name: "Logic",
    items: [
      {
        type: "conditional",
        label: "Conditional",
        description: "Branch based on condition",
        icon: GitBranch,
      },
      {
        type: "loop",
        label: "Loop",
        description: "Repeat steps in a loop",
        icon: Repeat,
      },
    ],
  },
  {
    name: "Integration",
    items: [
      {
        type: "skill",
        label: "Skill",
        description: "Execute a WASM skill",
        icon: Puzzle,
      },
      {
        type: "http",
        label: "HTTP Request",
        description: "Call an external API",
        icon: Globe,
      },
      {
        type: "code",
        label: "Code",
        description: "Run TypeScript or WASM",
        icon: Code2,
      },
    ],
  },
  {
    name: "Control",
    items: [
      {
        type: "approval",
        label: "Approval",
        description: "Wait for human approval",
        icon: ShieldCheck,
      },
      {
        type: "sub_workflow",
        label: "Sub-Workflow",
        description: "Invoke another workflow",
        icon: Workflow,
      },
    ],
  },
];

// ---------------------------------------------------------------------------
// Draggable palette item
// ---------------------------------------------------------------------------

function DraggablePaletteItem({ item }: { item: PaletteItem }) {
  const onDragStart = useCallback(
    (event: DragEvent<HTMLDivElement>) => {
      event.dataTransfer.setData(
        "application/boternity-step-type",
        item.type,
      );
      event.dataTransfer.effectAllowed = "move";
    },
    [item.type],
  );

  const Icon = item.icon;

  return (
    <div
      draggable
      onDragStart={onDragStart}
      className="flex items-center gap-3 p-2.5 rounded-md border bg-card cursor-grab hover:bg-accent/50 active:cursor-grabbing transition-colors"
    >
      <div className="shrink-0 flex items-center justify-center size-8 rounded bg-muted">
        <Icon className="size-4 text-muted-foreground" />
      </div>
      <div className="min-w-0">
        <p className="text-sm font-medium truncate">{item.label}</p>
        <p className="text-[11px] text-muted-foreground truncate">
          {item.description}
        </p>
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------
// NodePalette component
// ---------------------------------------------------------------------------

export function NodePalette() {
  return (
    <div className="w-60 border-r bg-card flex flex-col h-full shrink-0">
      <div className="p-3 border-b shrink-0">
        <h3 className="text-sm font-semibold">Steps</h3>
        <p className="text-[11px] text-muted-foreground mt-0.5">
          Drag onto canvas to add
        </p>
      </div>

      <ScrollArea className="flex-1 overflow-hidden">
        <div className="p-3 space-y-5">
          {PALETTE_CATEGORIES.map((category) => (
            <div key={category.name}>
              <h4 className="text-[10px] font-semibold text-muted-foreground uppercase tracking-wider mb-2">
                {category.name}
              </h4>
              <div className="space-y-1.5">
                {category.items.map((item) => (
                  <DraggablePaletteItem key={item.type} item={item} />
                ))}
              </div>
            </div>
          ))}
        </div>
      </ScrollArea>
    </div>
  );
}
