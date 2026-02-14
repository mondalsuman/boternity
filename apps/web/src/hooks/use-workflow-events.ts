/**
 * Hook for tracking live workflow execution events via WebSocket.
 *
 * Filters events by run_id and maintains a map of step statuses
 * (pending/running/completed/failed/skipped/waiting_approval).
 * Used by the workflow builder to update node status colors in real time.
 */

import { useState, useCallback, useEffect, useRef } from "react";
import type { WorkflowStepStatus, WorkflowRunStatus } from "@/types/workflow";

// ---------------------------------------------------------------------------
// Workflow event types (mirrors Rust AgentEvent workflow variants)
// ---------------------------------------------------------------------------

export interface WorkflowRunStartedEvent {
  type: "workflow_run_started";
  run_id: string;
  workflow_name: string;
  trigger_type: string;
}

export interface WorkflowStepStartedEvent {
  type: "workflow_step_started";
  run_id: string;
  step_id: string;
  step_name: string;
  step_type: string;
}

export interface WorkflowStepCompletedEvent {
  type: "workflow_step_completed";
  run_id: string;
  step_id: string;
  step_name: string;
  duration_ms: number;
}

export interface WorkflowStepFailedEvent {
  type: "workflow_step_failed";
  run_id: string;
  step_id: string;
  step_name: string;
  error: string;
  will_retry: boolean;
}

export interface WorkflowRunCompletedEvent {
  type: "workflow_run_completed";
  run_id: string;
  workflow_name: string;
  duration_ms: number;
  steps_completed: number;
}

export interface WorkflowRunFailedEvent {
  type: "workflow_run_failed";
  run_id: string;
  workflow_name: string;
  error: string;
}

export interface WorkflowRunPausedEvent {
  type: "workflow_run_paused";
  run_id: string;
  step_id: string;
  reason: string;
}

export type WorkflowEvent =
  | WorkflowRunStartedEvent
  | WorkflowStepStartedEvent
  | WorkflowStepCompletedEvent
  | WorkflowStepFailedEvent
  | WorkflowRunCompletedEvent
  | WorkflowRunFailedEvent
  | WorkflowRunPausedEvent;

// ---------------------------------------------------------------------------
// Step execution status tracking
// ---------------------------------------------------------------------------

export interface StepExecutionInfo {
  status: WorkflowStepStatus;
  duration_ms?: number;
  error?: string;
  will_retry?: boolean;
}

export interface WorkflowExecutionState {
  /** The current run ID being tracked. */
  runId: string | null;
  /** Overall run status. */
  runStatus: WorkflowRunStatus | null;
  /** Per-step execution status map (step_id -> info). */
  stepStatuses: Map<string, StepExecutionInfo>;
  /** Total run duration in ms (set on completion). */
  totalDuration: number | null;
  /** Total steps completed (set on completion). */
  totalStepsCompleted: number | null;
  /** Run-level error message (set on failure). */
  runError: string | null;
  /** Whether a run is currently active. */
  isRunning: boolean;
}

// ---------------------------------------------------------------------------
// Hook
// ---------------------------------------------------------------------------

type EventListener = (event: unknown) => void;

interface UseWorkflowEventsOptions {
  /**
   * Function to register an event listener on the WebSocket.
   * Returns an unsubscribe function.
   */
  onEvent: (fn: EventListener) => () => void;
  /**
   * Function to send a command to the WebSocket.
   */
  sendCommand: (cmd: object) => void;
}

/**
 * Hook that tracks live workflow execution events from the WebSocket.
 *
 * Usage:
 * ```ts
 * const { status, stepStatuses, startTracking, stopTracking } = useWorkflowEvents({
 *   onEvent: ws.onEvent,
 *   sendCommand: ws.sendCommand,
 * });
 * ```
 */
export function useWorkflowEvents({
  onEvent,
  sendCommand,
}: UseWorkflowEventsOptions) {
  const [state, setState] = useState<WorkflowExecutionState>({
    runId: null,
    runStatus: null,
    stepStatuses: new Map(),
    totalDuration: null,
    totalStepsCompleted: null,
    runError: null,
    isRunning: false,
  });

  const runIdRef = useRef<string | null>(null);

  /**
   * Start tracking events for a specific workflow run.
   * Resets state and subscribes to the run_id via WebSocket.
   */
  const startTracking = useCallback(
    (runId: string) => {
      runIdRef.current = runId;
      setState({
        runId,
        runStatus: "running",
        stepStatuses: new Map(),
        totalDuration: null,
        totalStepsCompleted: null,
        runError: null,
        isRunning: true,
      });

      // Subscribe to workflow events for this run
      sendCommand({ type: "subscribe_workflow", run_id: runId });
    },
    [sendCommand],
  );

  /**
   * Stop tracking events for the current run.
   */
  const stopTracking = useCallback(() => {
    if (runIdRef.current) {
      sendCommand({
        type: "unsubscribe_workflow",
        run_id: runIdRef.current,
      });
      runIdRef.current = null;
    }
    setState((prev) => ({
      ...prev,
      isRunning: false,
    }));
  }, [sendCommand]);

  /**
   * Reset execution state completely.
   */
  const reset = useCallback(() => {
    if (runIdRef.current) {
      sendCommand({
        type: "unsubscribe_workflow",
        run_id: runIdRef.current,
      });
      runIdRef.current = null;
    }
    setState({
      runId: null,
      runStatus: null,
      stepStatuses: new Map(),
      totalDuration: null,
      totalStepsCompleted: null,
      runError: null,
      isRunning: false,
    });
  }, [sendCommand]);

  // Process incoming workflow events
  useEffect(() => {
    const unsubscribe = onEvent((raw: unknown) => {
      const event = raw as WorkflowEvent;
      if (!event || typeof event !== "object" || !("type" in event)) return;

      // Filter: only process events matching tracked run_id
      if ("run_id" in event && event.run_id !== runIdRef.current) return;

      switch (event.type) {
        case "workflow_run_started":
          setState((prev) => ({
            ...prev,
            runStatus: "running",
            isRunning: true,
          }));
          break;

        case "workflow_step_started":
          setState((prev) => {
            const next = new Map(prev.stepStatuses);
            next.set(event.step_id, { status: "running" });
            return { ...prev, stepStatuses: next };
          });
          break;

        case "workflow_step_completed":
          setState((prev) => {
            const next = new Map(prev.stepStatuses);
            next.set(event.step_id, {
              status: "completed",
              duration_ms: event.duration_ms,
            });
            return { ...prev, stepStatuses: next };
          });
          break;

        case "workflow_step_failed":
          setState((prev) => {
            const next = new Map(prev.stepStatuses);
            next.set(event.step_id, {
              status: "failed",
              error: event.error,
              will_retry: event.will_retry,
            });
            return { ...prev, stepStatuses: next };
          });
          break;

        case "workflow_run_completed":
          setState((prev) => ({
            ...prev,
            runStatus: "completed",
            totalDuration: event.duration_ms,
            totalStepsCompleted: event.steps_completed,
            isRunning: false,
          }));
          break;

        case "workflow_run_failed":
          setState((prev) => ({
            ...prev,
            runStatus: "failed",
            runError: event.error,
            isRunning: false,
          }));
          break;

        case "workflow_run_paused":
          setState((prev) => {
            const next = new Map(prev.stepStatuses);
            next.set(event.step_id, {
              status: "waiting_approval",
            });
            return {
              ...prev,
              runStatus: "paused",
              stepStatuses: next,
              isRunning: false,
            };
          });
          break;
      }
    });

    return unsubscribe;
  }, [onEvent]);

  return {
    ...state,
    startTracking,
    stopTracking,
    reset,
  };
}
