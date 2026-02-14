/**
 * TypeScript types mirroring the canonical Rust `WorkflowDefinition` types
 * from `boternity-types::workflow`.
 *
 * These types are the single source of truth for the TypeScript SDK.
 * Any YAML generated from the builder pattern MUST conform to these types,
 * which in turn match the Rust serde representation.
 */

// ---------------------------------------------------------------------------
// Workflow Definition (canonical IR)
// ---------------------------------------------------------------------------

/** The canonical workflow definition. */
export interface WorkflowDefinition {
  /** UUIDv7 assigned on first save. */
  id: string;
  /** Human-readable workflow name. */
  name: string;
  /** Optional longer description. */
  description?: string;
  /** Semantic version string (e.g. "1.0.0"). */
  version: string;
  /** Who owns this workflow. */
  owner: WorkflowOwner;
  /** Maximum concurrent instances (undefined = unlimited). */
  concurrency?: number;
  /** Per-workflow timeout in seconds. */
  timeout_secs?: number;
  /** Trigger configurations. */
  triggers: TriggerConfig[];
  /** Ordered list of step definitions forming the workflow DAG. */
  steps: StepDefinition[];
  /** Extensible metadata. */
  metadata?: Record<string, unknown>;
}

// ---------------------------------------------------------------------------
// Owner
// ---------------------------------------------------------------------------

export type WorkflowOwner = WorkflowOwnerBot | WorkflowOwnerGlobal;

export interface WorkflowOwnerBot {
  type: "bot";
  bot_id: string;
  slug: string;
}

export interface WorkflowOwnerGlobal {
  type: "global";
}

// ---------------------------------------------------------------------------
// Step Definition
// ---------------------------------------------------------------------------

/** A single step in the workflow DAG. */
export interface StepDefinition {
  /** User-defined step ID (unique within a workflow). */
  id: string;
  /** Human-readable step name. */
  name: string;
  /** The kind of step. */
  type: StepType;
  /** Step IDs this step depends on (DAG edges). */
  depends_on?: string[];
  /** Optional JEXL expression for conditional execution. */
  condition?: string;
  /** Step-level timeout in seconds. */
  timeout_secs?: number;
  /** Retry configuration. */
  retry?: RetryConfig;
  /** Step-specific configuration payload. */
  config: StepConfig;
  /** Visual builder metadata. */
  ui?: StepUiMetadata;
}

/** The kind of step in a workflow. */
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
// Step Config (discriminated union on `type`)
// ---------------------------------------------------------------------------

export type StepConfig =
  | AgentStepConfig
  | SkillStepConfig
  | CodeStepConfig
  | HttpStepConfig
  | ConditionalStepConfig
  | LoopStepConfig
  | ApprovalStepConfig
  | SubWorkflowStepConfig;

export interface AgentStepConfig {
  type: "agent";
  bot: string;
  prompt: string;
  model?: string;
}

export interface SkillStepConfig {
  type: "skill";
  skill: string;
  input?: string;
}

export interface CodeStepConfig {
  type: "code";
  language: CodeLanguage;
  source: string;
}

export interface HttpStepConfig {
  type: "http";
  method: string;
  url: string;
  headers?: Record<string, string>;
  body?: string;
}

export interface ConditionalStepConfig {
  type: "conditional";
  condition: string;
  then_steps: string[];
  else_steps: string[];
}

export interface LoopStepConfig {
  type: "loop";
  condition: string;
  max_iterations?: number;
  body_steps: string[];
}

export interface ApprovalStepConfig {
  type: "approval";
  prompt: string;
  timeout_secs?: number;
}

export interface SubWorkflowStepConfig {
  type: "sub_workflow";
  workflow_name: string;
  input?: unknown;
}

export type CodeLanguage = "type_script" | "wasm";

// ---------------------------------------------------------------------------
// Retry Configuration
// ---------------------------------------------------------------------------

export interface RetryConfig {
  /** Maximum number of attempts (default 3). */
  max_attempts?: number;
  /** Retry strategy. */
  strategy: RetryStrategy;
}

export type RetryStrategy = "simple" | "llm_self_correct";

// ---------------------------------------------------------------------------
// Trigger Configuration
// ---------------------------------------------------------------------------

export type TriggerConfig =
  | ManualTrigger
  | CronTrigger
  | WebhookTrigger
  | EventTrigger
  | FileWatchTrigger;

export interface ManualTrigger {
  type: "manual";
}

export interface CronTrigger {
  type: "cron";
  schedule: string;
  timezone?: string;
}

export interface WebhookTrigger {
  type: "webhook";
  path: string;
  auth?: WebhookAuth;
  when?: string;
}

export interface EventTrigger {
  type: "event";
  source: string;
  event_type: string;
  when?: string;
}

export interface FileWatchTrigger {
  type: "file_watch";
  paths: string[];
  patterns?: string[];
  when?: string;
}

export type WebhookAuth = HmacSha256Auth | BearerTokenAuth;

export interface HmacSha256Auth {
  type: "hmac_sha256";
  secret_name: string;
}

export interface BearerTokenAuth {
  type: "bearer_token";
  secret_name: string;
}

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
