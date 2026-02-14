/**
 * Workflow API client for the web UI.
 *
 * Typed functions wrapping the REST endpoints from the workflow system.
 * All calls use the shared apiFetch wrapper for envelope unwrapping,
 * auth headers, and error handling.
 */

import { apiFetch } from "@/lib/api-client";
import type {
  WorkflowDefinition,
  WorkflowSummary,
  WorkflowRun,
  WorkflowStepLog,
} from "@/types/workflow";

// ---------------------------------------------------------------------------
// Workflow CRUD
// ---------------------------------------------------------------------------

/**
 * Fetch all workflows, optionally filtered by bot slug.
 */
export async function fetchWorkflows(
  botSlug?: string,
): Promise<WorkflowSummary[]> {
  const query = botSlug ? `?bot=${encodeURIComponent(botSlug)}` : "";
  return apiFetch<WorkflowSummary[]>(`/workflows${query}`);
}

/**
 * Fetch a single workflow definition by ID.
 */
export async function fetchWorkflow(
  id: string,
): Promise<WorkflowDefinition> {
  return apiFetch<WorkflowDefinition>(`/workflows/${id}`);
}

/**
 * Create a new workflow from a definition.
 */
export async function createWorkflow(
  def: Omit<WorkflowDefinition, "id">,
): Promise<{ id: string }> {
  return apiFetch<{ id: string }>("/workflows", {
    method: "POST",
    body: JSON.stringify(def),
  });
}

/**
 * Update an existing workflow definition.
 */
export async function updateWorkflow(
  id: string,
  def: WorkflowDefinition,
): Promise<void> {
  await apiFetch<unknown>(`/workflows/${id}`, {
    method: "PUT",
    body: JSON.stringify(def),
  });
}

/**
 * Delete a workflow by ID.
 */
export async function deleteWorkflow(id: string): Promise<void> {
  await apiFetch<unknown>(`/workflows/${id}`, {
    method: "DELETE",
  });
}

// ---------------------------------------------------------------------------
// Trigger
// ---------------------------------------------------------------------------

/**
 * Manually trigger a workflow run.
 */
export async function triggerWorkflow(
  id: string,
  payload?: unknown,
): Promise<{ run_id: string }> {
  return apiFetch<{ run_id: string }>(`/workflows/${id}/trigger`, {
    method: "POST",
    body: JSON.stringify(payload ?? {}),
  });
}

// ---------------------------------------------------------------------------
// Runs
// ---------------------------------------------------------------------------

/**
 * Fetch recent runs for a workflow.
 */
export async function fetchRuns(
  workflowId: string,
  limit?: number,
): Promise<WorkflowRun[]> {
  const query = limit ? `?limit=${limit}` : "";
  return apiFetch<WorkflowRun[]>(`/workflows/${workflowId}/runs${query}`);
}

/**
 * Fetch detail for a specific run including step logs.
 */
export async function fetchRunDetail(
  runId: string,
): Promise<{ run: WorkflowRun; steps: WorkflowStepLog[] }> {
  return apiFetch<{ run: WorkflowRun; steps: WorkflowStepLog[] }>(
    `/runs/${runId}`,
  );
}

// ---------------------------------------------------------------------------
// Run Actions
// ---------------------------------------------------------------------------

/**
 * Approve a paused workflow run (for approval gates).
 */
export async function approveRun(runId: string): Promise<void> {
  await apiFetch<unknown>(`/runs/${runId}/approve`, {
    method: "POST",
  });
}

/**
 * Cancel a running or paused workflow run.
 */
export async function cancelRun(runId: string): Promise<void> {
  await apiFetch<unknown>(`/runs/${runId}/cancel`, {
    method: "POST",
  });
}
