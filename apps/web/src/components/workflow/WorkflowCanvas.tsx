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
 * - onDrop creates new nodes from palette drag
 * - Node selection triggers config panel
 * - Keyboard shortcuts: Ctrl+Z (undo), Ctrl+Y / Ctrl+Shift+Z (redo)
 * - Node grouping via parentId
 *
 * Node/edge state managed via React Flow hooks (useNodesState, useEdgesState)
 * for optimal re-render performance.
 */

import { useCallback, useEffect, useRef, type DragEvent } from "react";
import {
  ReactFlow,
  MiniMap,
  Controls,
  Background,
  BackgroundVariant,
  useNodesState,
  useEdgesState,
  addEdge,
  useReactFlow,
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

import type { StepDefinition, StepType, WorkflowDefinition } from "@/types/workflow";
import { useUndoRedo } from "@/hooks/use-undo-redo";

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
// Default data for each step type (when created via palette drop)
// ---------------------------------------------------------------------------

function getDefaultNodeData(stepType: StepType): Record<string, unknown> {
  const base = { label: `New ${stepType}` };
  switch (stepType) {
    case "agent":
      return { ...base, bot: "", prompt: "" };
    case "skill":
      return { ...base, skill: "", input: "" };
    case "code":
      return { ...base, language: "type_script", source: "" };
    case "http":
      return { ...base, method: "GET", url: "" };
    case "conditional":
      return { ...base, condition: "", then_steps: [], else_steps: [] };
    case "loop":
      return { ...base, condition: "", max_iterations: 10, body_steps: [] };
    case "approval":
      return { ...base, prompt: "", timeout_secs: 3600 };
    case "sub_workflow":
      return { ...base, workflow_name: "", input: undefined };
  }
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
    ...(step.ui?.group ? { parentId: step.ui.group } : {}),
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
// Undo/redo snapshot type
// ---------------------------------------------------------------------------

interface CanvasSnapshot {
  nodes: Node[];
  edges: Edge[];
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
  /** Callback when a node is clicked/selected. */
  onNodeSelect?: (node: Node | null) => void;
  /** Expose auto-layout function to parent. */
  onAutoLayout?: () => void;
  /** Ref for imperative methods (autoLayout, undo, redo, groupSelected, ungroupSelected). */
  canvasRef?: React.Ref<WorkflowCanvasHandle>;
}

export interface WorkflowCanvasHandle {
  autoLayout: () => void;
  undo: () => void;
  redo: () => void;
  canUndo: boolean;
  canRedo: boolean;
  groupSelected: () => void;
  ungroupSelected: () => void;
}

export function WorkflowCanvas({
  initialNodes,
  initialEdges,
  onChange,
  onNodeSelect,
  canvasRef,
}: WorkflowCanvasProps) {
  const [nodes, setNodes, onNodesChange] = useNodesState(initialNodes);
  const [edges, setEdges, onEdgesChange] = useEdgesState(initialEdges);
  const { screenToFlowPosition } = useReactFlow();
  const wrapperRef = useRef<HTMLDivElement>(null);
  const undoRedo = useUndoRedo<CanvasSnapshot>();

  // Snapshot helper
  const takeSnapshot = useCallback(() => {
    undoRedo.takeSnapshot({ nodes, edges });
  }, [nodes, edges, undoRedo]);

  const onConnect: OnConnect = useCallback(
    (connection: Connection) => {
      takeSnapshot();
      setEdges((eds) => {
        const newEdges = addEdge(
          { ...connection, type: "typed", data: { dataType: "default" } },
          eds,
        );
        return newEdges;
      });
    },
    [setEdges, takeSnapshot],
  );

  /** Re-layout all nodes using dagre. */
  const handleAutoLayout = useCallback(() => {
    takeSnapshot();
    const { nodes: layouted, edges: layoutedEdges } = getLayoutedElements(
      nodes,
      edges,
    );
    setNodes(layouted);
    setEdges(layoutedEdges);
    onChange?.(layouted, layoutedEdges);
  }, [nodes, edges, setNodes, setEdges, onChange, takeSnapshot]);

  // -------------------------------------------------------------------------
  // Drop handler: create new node from palette drag
  // -------------------------------------------------------------------------

  const onDragOver = useCallback((event: DragEvent<HTMLDivElement>) => {
    event.preventDefault();
    event.dataTransfer.dropEffect = "move";
  }, []);

  const onDrop = useCallback(
    (event: DragEvent<HTMLDivElement>) => {
      event.preventDefault();

      const stepType = event.dataTransfer.getData(
        "application/boternity-step-type",
      ) as StepType | "";

      if (!stepType) return;

      const position = screenToFlowPosition({
        x: event.clientX,
        y: event.clientY,
      });

      takeSnapshot();

      const newNode: Node = {
        id: `${stepType}-${Date.now()}`,
        type: stepType,
        position,
        data: getDefaultNodeData(stepType),
      };

      setNodes((nds) => [...nds, newNode]);
      onChange?.(
        [...nodes, newNode],
        edges,
      );
    },
    [screenToFlowPosition, setNodes, nodes, edges, onChange, takeSnapshot],
  );

  // -------------------------------------------------------------------------
  // Node selection
  // -------------------------------------------------------------------------

  const handleNodeClick = useCallback(
    (_event: React.MouseEvent, node: Node) => {
      onNodeSelect?.(node);
    },
    [onNodeSelect],
  );

  const handlePaneClick = useCallback(() => {
    onNodeSelect?.(null);
  }, [onNodeSelect]);

  // -------------------------------------------------------------------------
  // Undo / Redo
  // -------------------------------------------------------------------------

  const handleUndo = useCallback(() => {
    const snapshot = undoRedo.undo({ nodes, edges });
    if (snapshot) {
      setNodes(snapshot.nodes);
      setEdges(snapshot.edges);
      onChange?.(snapshot.nodes, snapshot.edges);
    }
  }, [undoRedo, nodes, edges, setNodes, setEdges, onChange]);

  const handleRedo = useCallback(() => {
    const snapshot = undoRedo.redo({ nodes, edges });
    if (snapshot) {
      setNodes(snapshot.nodes);
      setEdges(snapshot.edges);
      onChange?.(snapshot.nodes, snapshot.edges);
    }
  }, [undoRedo, nodes, edges, setNodes, setEdges, onChange]);

  // Keyboard shortcuts
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      const isCtrlOrMeta = e.ctrlKey || e.metaKey;
      if (!isCtrlOrMeta) return;

      if (e.key === "z" && !e.shiftKey) {
        e.preventDefault();
        handleUndo();
      } else if (e.key === "y" || (e.key === "z" && e.shiftKey)) {
        e.preventDefault();
        handleRedo();
      }
    };

    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [handleUndo, handleRedo]);

  // -------------------------------------------------------------------------
  // Node grouping
  // -------------------------------------------------------------------------

  const groupSelected = useCallback(() => {
    const selected = nodes.filter((n) => n.selected);
    if (selected.length < 2) return;

    takeSnapshot();

    const groupId = `group-${Date.now()}`;

    // Calculate bounding box of selected nodes
    const minX = Math.min(...selected.map((n) => n.position.x));
    const minY = Math.min(...selected.map((n) => n.position.y));
    const maxX = Math.max(...selected.map((n) => n.position.x + NODE_WIDTH));
    const maxY = Math.max(...selected.map((n) => n.position.y + NODE_HEIGHT));

    const padding = 40;

    // Create group node
    const groupNode: Node = {
      id: groupId,
      type: "group",
      position: { x: minX - padding, y: minY - padding },
      data: { label: "Group" },
      style: {
        width: maxX - minX + padding * 2,
        height: maxY - minY + padding * 2,
        backgroundColor: "rgba(100, 100, 200, 0.05)",
        borderRadius: "8px",
        border: "2px dashed rgba(100, 100, 200, 0.3)",
      },
    };

    // Re-parent selected nodes
    const updatedNodes = nodes.map((n) => {
      if (n.selected) {
        return {
          ...n,
          parentId: groupId,
          position: {
            x: n.position.x - groupNode.position.x,
            y: n.position.y - groupNode.position.y,
          },
        };
      }
      return n;
    });

    const allNodes = [groupNode, ...updatedNodes];
    setNodes(allNodes);
    onChange?.(allNodes, edges);
  }, [nodes, edges, setNodes, onChange, takeSnapshot]);

  const ungroupSelected = useCallback(() => {
    const selectedGroups = nodes.filter(
      (n) => n.selected && n.type === "group",
    );
    if (selectedGroups.length === 0) return;

    takeSnapshot();

    const groupIds = new Set(selectedGroups.map((g) => g.id));
    const groupPositions = new Map(
      selectedGroups.map((g) => [g.id, g.position]),
    );

    const updatedNodes = nodes
      .filter((n) => !groupIds.has(n.id))
      .map((n) => {
        if (n.parentId && groupIds.has(n.parentId)) {
          const parentPos = groupPositions.get(n.parentId)!;
          return {
            ...n,
            parentId: undefined,
            position: {
              x: n.position.x + parentPos.x,
              y: n.position.y + parentPos.y,
            },
          };
        }
        return n;
      });

    setNodes(updatedNodes);
    onChange?.(updatedNodes, edges);
  }, [nodes, edges, setNodes, onChange, takeSnapshot]);

  // -------------------------------------------------------------------------
  // Imperative handle
  // -------------------------------------------------------------------------

  useEffect(() => {
    if (!canvasRef) return;

    const handle: WorkflowCanvasHandle = {
      autoLayout: handleAutoLayout,
      undo: handleUndo,
      redo: handleRedo,
      canUndo: undoRedo.canUndo,
      canRedo: undoRedo.canRedo,
      groupSelected,
      ungroupSelected,
    };

    if (typeof canvasRef === "function") {
      canvasRef(handle as unknown as WorkflowCanvasHandle);
    } else if (canvasRef && "current" in canvasRef) {
      (canvasRef as React.MutableRefObject<WorkflowCanvasHandle | null>).current = handle;
    }
  }, [canvasRef, handleAutoLayout, handleUndo, handleRedo, undoRedo.canUndo, undoRedo.canRedo, groupSelected, ungroupSelected]);

  return (
    <div ref={wrapperRef} className="w-full h-full relative">
      <ReactFlow
        nodes={nodes}
        edges={edges}
        onNodesChange={(changes) => {
          takeSnapshot();
          onNodesChange(changes);
          onChange?.(nodes, edges);
        }}
        onEdgesChange={(changes) => {
          takeSnapshot();
          onEdgesChange(changes);
          onChange?.(nodes, edges);
        }}
        onConnect={onConnect}
        onNodeClick={handleNodeClick}
        onPaneClick={handlePaneClick}
        onDrop={onDrop}
        onDragOver={onDragOver}
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
    </div>
  );
}
