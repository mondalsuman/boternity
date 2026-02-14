/**
 * React Flow canvas for the visual workflow builder.
 *
 * Features:
 * - Custom nodeTypes for all 8 step types (registered as constant)
 * - Custom edgeTypes with TypedEdge color coding
 * - MiniMap, Controls, Background (dots grid)
 * - Dagre auto-layout (top-to-bottom)
 * - fitView on initial load
 * - onConnect creates typed edges
 *
 * Node/edge state managed via React Flow hooks (useNodesState, useEdgesState)
 * for optimal re-render performance.
 */

import { useCallback } from "react";
import {
  ReactFlow,
  MiniMap,
  Controls,
  Background,
  BackgroundVariant,
  useNodesState,
  useEdgesState,
  addEdge,
} from "@xyflow/react";
import type {
  Node,
  Edge,
  Connection,
  OnConnect,
  NodeTypes,
  EdgeTypes,
} from "@xyflow/react";
import dagre from "@dagrejs/dagre";
import "@xyflow/react/dist/style.css";

import { AgentNode } from "./nodes/AgentNode";
import { SkillNode } from "./nodes/SkillNode";
import { CodeNode } from "./nodes/CodeNode";
import { HttpNode } from "./nodes/HttpNode";
import { ConditionalNode } from "./nodes/ConditionalNode";
import { LoopNode } from "./nodes/LoopNode";
import { ApprovalNode } from "./nodes/ApprovalNode";
import { SubWorkflowNode } from "./nodes/SubWorkflowNode";
import { TypedEdge } from "./edges/TypedEdge";

import type { StepDefinition, WorkflowDefinition } from "@/types/workflow";

// ---------------------------------------------------------------------------
// Constant nodeTypes/edgeTypes (defined outside component to avoid re-renders)
// ---------------------------------------------------------------------------

const nodeTypes: NodeTypes = {
  agent: AgentNode,
  skill: SkillNode,
  code: CodeNode,
  http: HttpNode,
  conditional: ConditionalNode,
  loop: LoopNode,
  approval: ApprovalNode,
  sub_workflow: SubWorkflowNode,
};

const edgeTypes: EdgeTypes = {
  typed: TypedEdge,
};

// ---------------------------------------------------------------------------
// Dagre auto-layout
// ---------------------------------------------------------------------------

const NODE_WIDTH = 250;
const NODE_HEIGHT = 80;

/**
 * Apply dagre layout to position nodes in a top-to-bottom DAG.
 */
function getLayoutedElements(
  nodes: Node[],
  edges: Edge[],
  direction = "TB",
): { nodes: Node[]; edges: Edge[] } {
  const g = new dagre.graphlib.Graph();
  g.setDefaultEdgeLabel(() => ({}));
  g.setGraph({ rankdir: direction, nodesep: 50, ranksep: 100 });

  for (const node of nodes) {
    g.setNode(node.id, { width: NODE_WIDTH, height: NODE_HEIGHT });
  }

  for (const edge of edges) {
    g.setEdge(edge.source, edge.target);
  }

  dagre.layout(g);

  const layoutedNodes = nodes.map((node) => {
    const pos = g.node(node.id);
    return {
      ...node,
      position: {
        x: pos.x - NODE_WIDTH / 2,
        y: pos.y - NODE_HEIGHT / 2,
      },
    };
  });

  return { nodes: layoutedNodes, edges };
}

// ---------------------------------------------------------------------------
// Definition <-> Flow conversion helpers
// ---------------------------------------------------------------------------

/**
 * Convert a WorkflowDefinition to React Flow nodes and edges.
 */
export function definitionToFlow(def: WorkflowDefinition): {
  nodes: Node[];
  edges: Edge[];
} {
  const nodes: Node[] = def.steps.map((step) => ({
    id: step.id,
    type: step.type,
    position: step.ui?.position ?? { x: 0, y: 0 },
    data: {
      label: step.name,
      ...extractStepData(step),
    },
  }));

  const edges: Edge[] = [];
  for (const step of def.steps) {
    for (const dep of step.depends_on) {
      edges.push({
        id: `${dep}->${step.id}`,
        source: dep,
        target: step.id,
        type: "typed",
        data: { dataType: "default" },
      });
    }

    // Conditional nodes: add edges to then/else steps
    if (step.config.type === "conditional") {
      for (const thenStep of step.config.then_steps) {
        edges.push({
          id: `${step.id}->then-${thenStep}`,
          source: step.id,
          target: thenStep,
          sourceHandle: "then",
          type: "typed",
          data: { dataType: "json" },
        });
      }
      for (const elseStep of step.config.else_steps) {
        edges.push({
          id: `${step.id}->else-${elseStep}`,
          source: step.id,
          target: elseStep,
          sourceHandle: "else",
          type: "typed",
          data: { dataType: "json" },
        });
      }
    }

    // Loop nodes: add edges to body steps
    if (step.config.type === "loop") {
      for (const bodyStep of step.config.body_steps) {
        edges.push({
          id: `${step.id}->body-${bodyStep}`,
          source: step.id,
          target: bodyStep,
          type: "typed",
          data: { dataType: "json" },
        });
      }
    }
  }

  // Check if any nodes have positions from the UI metadata
  const hasPositions = nodes.some(
    (n) => n.position.x !== 0 || n.position.y !== 0,
  );

  if (!hasPositions) {
    return getLayoutedElements(nodes, edges);
  }

  return { nodes, edges };
}

/**
 * Extract step-type-specific data for the custom node.
 */
function extractStepData(
  step: StepDefinition,
): Record<string, unknown> {
  const config = step.config;
  switch (config.type) {
    case "agent":
      return { bot: config.bot, prompt: config.prompt, model: config.model };
    case "skill":
      return { skill: config.skill, input: config.input };
    case "code":
      return { language: config.language, source: config.source };
    case "http":
      return { method: config.method, url: config.url };
    case "conditional":
      return {
        condition: config.condition,
        then_steps: config.then_steps,
        else_steps: config.else_steps,
      };
    case "loop":
      return {
        condition: config.condition,
        max_iterations: config.max_iterations,
        body_steps: config.body_steps,
      };
    case "approval":
      return { prompt: config.prompt, timeout_secs: config.timeout_secs };
    case "sub_workflow":
      return {
        workflow_name: config.workflow_name,
        input: config.input,
      };
  }
}

/**
 * Convert React Flow nodes/edges back to step definitions for saving.
 * Preserves the original definition structure and updates positions.
 */
export function flowToDefinition(
  originalDef: WorkflowDefinition,
  nodes: Node[],
  _edges: Edge[],
): WorkflowDefinition {
  const positionMap = new Map(
    nodes.map((n) => [n.id, { x: n.position.x, y: n.position.y }]),
  );

  return {
    ...originalDef,
    steps: originalDef.steps.map((step) => {
      const pos = positionMap.get(step.id);
      return {
        ...step,
        ui: pos
          ? { ...step.ui, position: pos }
          : step.ui,
      };
    }),
  };
}

// ---------------------------------------------------------------------------
// WorkflowCanvas Component
// ---------------------------------------------------------------------------

interface WorkflowCanvasProps {
  /** Initial nodes (from definitionToFlow). */
  initialNodes: Node[];
  /** Initial edges (from definitionToFlow). */
  initialEdges: Edge[];
  /** Callback when nodes/edges change (for parent save state). */
  onChange?: (nodes: Node[], edges: Edge[]) => void;
}

export function WorkflowCanvas({
  initialNodes,
  initialEdges,
  onChange,
}: WorkflowCanvasProps) {
  const [nodes, setNodes, onNodesChange] = useNodesState(initialNodes);
  const [edges, setEdges, onEdgesChange] = useEdgesState(initialEdges);

  const onConnect: OnConnect = useCallback(
    (connection: Connection) => {
      setEdges((eds) => {
        const newEdges = addEdge(
          { ...connection, type: "typed", data: { dataType: "default" } },
          eds,
        );
        return newEdges;
      });
    },
    [setEdges],
  );

  /** Re-layout all nodes using dagre. */
  const handleAutoLayout = useCallback(() => {
    const { nodes: layouted, edges: layoutedEdges } = getLayoutedElements(
      nodes,
      edges,
    );
    setNodes(layouted);
    setEdges(layoutedEdges);
    onChange?.(layouted, layoutedEdges);
  }, [nodes, edges, setNodes, setEdges, onChange]);

  return (
    <div className="w-full h-full relative">
      <ReactFlow
        nodes={nodes}
        edges={edges}
        onNodesChange={(changes) => {
          onNodesChange(changes);
          onChange?.(nodes, edges);
        }}
        onEdgesChange={(changes) => {
          onEdgesChange(changes);
          onChange?.(nodes, edges);
        }}
        onConnect={onConnect}
        nodeTypes={nodeTypes}
        edgeTypes={edgeTypes}
        fitView
        fitViewOptions={{ padding: 0.2 }}
        minZoom={0.1}
        maxZoom={2}
        defaultEdgeOptions={{ type: "typed" }}
      >
        <Background variant={BackgroundVariant.Dots} gap={16} size={1} />
        <Controls showInteractive={false} />
        <MiniMap
          zoomable
          pannable
          className="!bg-card !border-border"
        />
      </ReactFlow>

      {/* Auto-layout button */}
      <button
        onClick={handleAutoLayout}
        className="absolute top-3 right-3 z-10 bg-card border rounded-md px-3 py-1.5 text-xs font-medium hover:bg-accent transition-colors shadow-sm"
      >
        Auto Layout
      </button>
    </div>
  );
}
