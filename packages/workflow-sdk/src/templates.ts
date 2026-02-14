/**
 * Pre-built workflow templates for common patterns.
 *
 * These functions return configured `WorkflowBuilder` instances that can be
 * further customized before calling `.build()` or `.toYaml()`.
 */

import { WorkflowBuilder, workflow } from "./builder.js";
import type { WorkflowOwner } from "./types.js";

// ---------------------------------------------------------------------------
// Data Pipeline template
// ---------------------------------------------------------------------------

export interface DataPipelineOptions {
  /** Workflow name. */
  name: string;
  /** Workflow owner. */
  owner?: WorkflowOwner;
  /** Bot slug for the collector agent. */
  collectorBot: string;
  /** Prompt for the collector agent. */
  collectorPrompt: string;
  /** Bot slug for the processor agent. */
  processorBot: string;
  /** Prompt for the processor agent. */
  processorPrompt: string;
  /** Optional HTTP endpoint to send results. */
  outputUrl?: string;
  /** Cron schedule for automatic runs. */
  schedule?: string;
}

/**
 * Create a data pipeline workflow: collect -> process -> (optionally) send.
 *
 * ```ts
 * const pipeline = dataPipeline({
 *   name: "news-pipeline",
 *   collectorBot: "researcher",
 *   collectorPrompt: "Find top 5 AI news stories",
 *   processorBot: "analyst",
 *   processorPrompt: "Summarize and rank the stories",
 *   outputUrl: "https://hooks.slack.com/xxx",
 *   schedule: "0 9 * * *",
 * });
 * ```
 */
export function dataPipeline(opts: DataPipelineOptions): WorkflowBuilder {
  const builder = workflow(opts.name)
    .description(`Data pipeline: collect with ${opts.collectorBot}, process with ${opts.processorBot}`)
    .trigger({ type: "manual" });

  if (opts.owner) builder.owner(opts.owner);
  if (opts.schedule) {
    builder.trigger({ type: "cron", schedule: opts.schedule });
  }

  const collect = builder.agent("collect", "Collect Data", {
    bot: opts.collectorBot,
    prompt: opts.collectorPrompt,
  });

  const process = builder.agent("process", "Process Data", {
    bot: opts.processorBot,
    prompt: opts.processorPrompt,
  }, { depends_on: [collect] });

  if (opts.outputUrl) {
    builder.http("send-output", "Send Output", {
      method: "POST",
      url: opts.outputUrl,
      headers: { "Content-Type": "application/json" },
      body: '{{ steps.process.output }}',
    }, { depends_on: [process] });
  }

  return builder;
}

// ---------------------------------------------------------------------------
// Approval Flow template
// ---------------------------------------------------------------------------

export interface ApprovalFlowOptions {
  /** Workflow name. */
  name: string;
  /** Workflow owner. */
  owner?: WorkflowOwner;
  /** Bot slug for the generator agent. */
  generatorBot: string;
  /** Prompt for the generator agent. */
  generatorPrompt: string;
  /** Prompt shown to the human reviewer. */
  reviewPrompt?: string;
  /** Review timeout in seconds (default 3600 = 1 hour). */
  reviewTimeout?: number;
  /** Bot slug for the publisher agent (runs after approval). */
  publisherBot: string;
  /** Prompt for the publisher agent. */
  publisherPrompt: string;
}

/**
 * Create an approval flow: generate -> human review -> publish.
 *
 * ```ts
 * const flow = approvalFlow({
 *   name: "content-review",
 *   generatorBot: "writer",
 *   generatorPrompt: "Draft a blog post about AI trends",
 *   publisherBot: "publisher",
 *   publisherPrompt: "Publish the approved content",
 * });
 * ```
 */
export function approvalFlow(opts: ApprovalFlowOptions): WorkflowBuilder {
  const builder = workflow(opts.name)
    .description(`Approval flow: generate with ${opts.generatorBot}, review, publish with ${opts.publisherBot}`)
    .trigger({ type: "manual" });

  if (opts.owner) builder.owner(opts.owner);

  const generate = builder.agent("generate", "Generate Content", {
    bot: opts.generatorBot,
    prompt: opts.generatorPrompt,
  });

  const review = builder.approval("review", "Human Review", {
    prompt: opts.reviewPrompt ?? "Review the generated content before publishing",
    timeout_secs: opts.reviewTimeout ?? 3600,
  }, { depends_on: [generate] });

  builder.agent("publish", "Publish", {
    bot: opts.publisherBot,
    prompt: opts.publisherPrompt,
  }, {
    depends_on: [review],
    condition: "context.approved == true",
  });

  return builder;
}

// ---------------------------------------------------------------------------
// Multi-Bot Collaboration template
// ---------------------------------------------------------------------------

export interface MultiBotCollaborationOptions {
  /** Workflow name. */
  name: string;
  /** Workflow owner. */
  owner?: WorkflowOwner;
  /**
   * List of parallel worker bots. Each runs independently and their
   * outputs are merged by the coordinator.
   */
  workers: Array<{
    id: string;
    name: string;
    bot: string;
    prompt: string;
  }>;
  /** Bot slug for the coordinator that merges worker outputs. */
  coordinatorBot: string;
  /** Prompt for the coordinator agent. */
  coordinatorPrompt: string;
  /** Maximum concurrent workers. */
  concurrency?: number;
}

/**
 * Create a multi-bot collaboration workflow:
 * parallel workers -> coordinator merges results.
 *
 * ```ts
 * const collab = multiBotCollaboration({
 *   name: "research-team",
 *   workers: [
 *     { id: "tech", name: "Tech Research", bot: "tech-researcher", prompt: "Research tech trends" },
 *     { id: "market", name: "Market Research", bot: "market-analyst", prompt: "Analyze market data" },
 *     { id: "social", name: "Social Analysis", bot: "social-analyst", prompt: "Analyze social trends" },
 *   ],
 *   coordinatorBot: "coordinator",
 *   coordinatorPrompt: "Synthesize all research into a comprehensive report",
 * });
 * ```
 */
export function multiBotCollaboration(
  opts: MultiBotCollaborationOptions,
): WorkflowBuilder {
  const builder = workflow(opts.name)
    .description(
      `Multi-bot collaboration: ${opts.workers.length} workers -> ${opts.coordinatorBot} coordinator`,
    )
    .trigger({ type: "manual" });

  if (opts.owner) builder.owner(opts.owner);
  if (opts.concurrency) builder.concurrency(opts.concurrency);

  // Add all workers (no dependencies -- they run in parallel)
  const workerRefs = opts.workers.map((w) =>
    builder.agent(w.id, w.name, { bot: w.bot, prompt: w.prompt }),
  );

  // Coordinator depends on all workers
  builder.agent("coordinate", "Coordinate Results", {
    bot: opts.coordinatorBot,
    prompt: opts.coordinatorPrompt,
  }, { depends_on: workerRefs });

  return builder;
}
