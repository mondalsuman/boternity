/**
 * Workflow domain types mirroring Rust crates/boternity-types/src/workflow.rs.
 *
 * All types use snake_case to match the Rust serde representation exactly.
 * Discriminated unions use the `type` field consistent with 08-12 conventions.
 */

// ---------------------------------------------------------------------------
// Workflow Definition (canonical IR)
// ---------------------------------------------------------------------------

export interface WorkflowDefinition {
  id: string;
  name: string;
  description?: string;
  version: string;
  owner: WorkflowOwner;
  concurrency?: number;
  timeout_secs?: number;
  triggers: TriggerConfig[];
  steps: StepDefinition[];
  metadata?: Record<string, unknown>;
}

/** Summary type for list views (lighter than full definition). */
export interface WorkflowSummary {
  id: string;
  name: string;
  description?: string;
  version: string;
  owner: WorkflowOwner;
  trigger_count: number;
  step_count: number;
  last_run_status?: WorkflowRunStatus;
  last_run_at?: string;
  created_at: string;
  updated_at: string;
}

// ---------------------------------------------------------------------------
// Owner
// ---------------------------------------------------------------------------

export type WorkflowOwner =
  | { type: "bot"; bot_id: string; slug: string }
  | { type: "global" };

// ---------------------------------------------------------------------------
// Step Definition
// ---------------------------------------------------------------------------

export interface StepDefinition {
  id: string;
  name: string;
  type: StepType;
  depends_on: string[];
  condition?: string;
  timeout_secs?: number;
  retry?: RetryConfig;
  config: StepConfig;
  ui?: StepUiMetadata;
}

export type StepType =
  | "agent"
  | "skill"
  | "code"
  | "http"
  | "conditional"
  | "loop"
  | "approval"
  | "sub_workflow";

// ---------------------------------------------------------------------------
// Step Config (tagged union on `type`)
// ---------------------------------------------------------------------------

export type StepConfig =
  | { type: "agent"; bot: string; prompt: string; model?: string }
  | { type: "skill"; skill: string; input?: string }
  | { type: "code"; language: CodeLanguage; source: string }
  | {
      type: "http";
      method: string;
      url: string;
      headers?: Record<string, string>;
      body?: string;
    }
  | {
      type: "conditional";
      condition: string;
      then_steps: string[];
      else_steps: string[];
    }
  | {
      type: "loop";
      condition: string;
      max_iterations?: number;
      body_steps: string[];
    }
  | { type: "approval"; prompt: string; timeout_secs?: number }
  | {
      type: "sub_workflow";
      workflow_name: string;
      input?: unknown;
    };

export type CodeLanguage = "type_script" | "wasm";

// ---------------------------------------------------------------------------
// Retry
// ---------------------------------------------------------------------------

export interface RetryConfig {
  max_attempts: number;
  strategy: RetryStrategy;
}

export type RetryStrategy = "simple" | "llm_self_correct";

// ---------------------------------------------------------------------------
// Trigger Config (tagged union on `type`)
// ---------------------------------------------------------------------------

export type TriggerConfig =
  | { type: "manual" }
  | { type: "cron"; schedule: string; timezone?: string }
  | {
      type: "webhook";
      path: string;
      auth?: WebhookAuth;
      when?: string;
    }
  | {
      type: "event";
      source: string;
      event_type: string;
      when?: string;
    }
  | {
      type: "file_watch";
      paths: string[];
      patterns?: string[];
      when?: string;
    };

export type WebhookAuth =
  | { type: "hmac_sha256"; secret_name: string }
  | { type: "bearer_token"; secret_name: string };

// ---------------------------------------------------------------------------
// Visual Builder Metadata
// ---------------------------------------------------------------------------

export interface StepUiMetadata {
  position?: UiPosition;
  group?: string;
  collapsed?: boolean;
}

export interface UiPosition {
  x: number;
  y: number;
}

// ---------------------------------------------------------------------------
// Run Status
// ---------------------------------------------------------------------------

export type WorkflowRunStatus =
  | "pending"
  | "running"
  | "paused"
  | "completed"
  | "failed"
  | "crashed"
  | "cancelled";

export type WorkflowStepStatus =
  | "pending"
  | "running"
  | "completed"
  | "failed"
  | "skipped"
  | "waiting_approval";

// ---------------------------------------------------------------------------
// Workflow Run (execution record)
// ---------------------------------------------------------------------------

export interface WorkflowRun {
  id: string;
  workflow_id: string;
  workflow_name: string;
  status: WorkflowRunStatus;
  trigger_type: string;
  trigger_payload?: unknown;
  context: unknown;
  started_at: string;
  completed_at?: string;
  error?: string;
  concurrency_key?: string;
}

// ---------------------------------------------------------------------------
// Step Execution Log
// ---------------------------------------------------------------------------

export interface WorkflowStepLog {
  id: string;
  run_id: string;
  step_id: string;
  step_name: string;
  status: WorkflowStepStatus;
  attempt: number;
  idempotency_key?: string;
  input?: unknown;
  output?: unknown;
  error?: string;
  started_at?: string;
  completed_at?: string;
}
