/**
 * @boternity/workflow-sdk
 *
 * TypeScript SDK for programmatic Boternity workflow definition.
 * Build workflows with a fluent builder pattern and generate valid YAML.
 *
 * @example
 * ```ts
 * import { workflow } from "@boternity/workflow-sdk";
 *
 * const wf = workflow("daily-digest")
 *   .version("1.0.0")
 *   .trigger({ type: "cron", schedule: "0 9 * * *" })
 *   .agent("gather", "Gather News", { bot: "researcher", prompt: "Find top 5 AI news" })
 *   .agent("analyze", "Analyze", { bot: "analyst", prompt: "Analyze trends" }, { depends_on: ["gather"] })
 *   .toYaml();
 * ```
 *
 * @packageDocumentation
 */

// Types (mirroring Rust WorkflowDefinition)
export type {
  WorkflowDefinition,
  WorkflowOwner,
  WorkflowOwnerBot,
  WorkflowOwnerGlobal,
  StepDefinition,
  StepType,
  StepConfig,
  AgentStepConfig,
  SkillStepConfig,
  CodeStepConfig,
  HttpStepConfig,
  ConditionalStepConfig,
  LoopStepConfig,
  ApprovalStepConfig,
  SubWorkflowStepConfig,
  CodeLanguage,
  RetryConfig,
  RetryStrategy,
  TriggerConfig,
  ManualTrigger,
  CronTrigger,
  WebhookTrigger,
  EventTrigger,
  FileWatchTrigger,
  WebhookAuth,
  HmacSha256Auth,
  BearerTokenAuth,
  StepUiMetadata,
  UiPosition,
} from "./types.js";

// Builder
export { workflow, WorkflowBuilder, StepRef } from "./builder.js";
export type { StepOptions } from "./builder.js";

// Templates
export {
  dataPipeline,
  approvalFlow,
  multiBotCollaboration,
} from "./templates.js";
export type {
  DataPipelineOptions,
  ApprovalFlowOptions,
  MultiBotCollaborationOptions,
} from "./templates.js";
