/**
 * Zustand store for workflow canvas state and UI selection tracking.
 *
 * Manages selected workflow/run IDs and builder open state.
 * Canvas node/edge state is managed locally in WorkflowCanvas
 * via React Flow's own hooks for optimal re-render performance.
 */

import { create } from "zustand";

interface WorkflowStore {
  /** Currently selected workflow ID (for list highlighting). */
  selectedWorkflowId: string | null;
  /** Currently selected run ID (for run detail expansion). */
  selectedRunId: string | null;
  /** Whether the visual builder is open. */
  isBuilderOpen: boolean;

  // Actions
  setSelectedWorkflow: (id: string | null) => void;
  setSelectedRun: (id: string | null) => void;
  openBuilder: () => void;
  closeBuilder: () => void;
  reset: () => void;
}

const initialState = {
  selectedWorkflowId: null as string | null,
  selectedRunId: null as string | null,
  isBuilderOpen: false,
};

export const useWorkflowStore = create<WorkflowStore>()((set) => ({
  ...initialState,

  setSelectedWorkflow: (id: string | null) =>
    set({ selectedWorkflowId: id }),

  setSelectedRun: (id: string | null) =>
    set({ selectedRunId: id }),

  openBuilder: () =>
    set({ isBuilderOpen: true }),

  closeBuilder: () =>
    set({ isBuilderOpen: false }),

  reset: () =>
    set(initialState),
}));
