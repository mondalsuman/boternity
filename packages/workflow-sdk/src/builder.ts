/**
 * Fluent builder pattern for constructing `WorkflowDefinition` objects.
 *
 * Usage:
 * ```ts
 * import { workflow } from "@boternity/workflow-sdk";
 *
 * const wf = workflow("daily-digest")
 *   .description("Gather news and summarize")
 *   .version("1.0.0")
 *   .owner({ type: "bot", bot_id: "...", slug: "researcher" })
 *   .concurrency(1)
 *   .trigger({ type: "cron", schedule: "0 9 * * *" })
 *   .trigger({ type: "manual" })
 *   .agent("gather", "Gather News", { bot: "researcher", prompt: "Find top 5 AI news" })
 *   .agent("analyze", "Analyze", { bot: "analyst", prompt: "Analyze trends" }, { depends_on: ["gather"] })
 *   .build();
 * ```
 */

import { stringify } from "yaml";
import type {
  WorkflowDefinition,
  WorkflowOwner,
  StepDefinition,
  StepType,
  StepConfig,
  TriggerConfig,
  RetryConfig,
  StepUiMetadata,
  CodeLanguage,
} from "./types.js";

// ---------------------------------------------------------------------------
// StepRef: type-safe reference to a step added to the builder
// ---------------------------------------------------------------------------

/**
 * A typed reference to a step that has been added to the builder.
 * Used for type-safe dependency tracking.
 */
export class StepRef {
  constructor(public readonly id: string) {}

  toString(): string {
    return this.id;
  }
}

// ---------------------------------------------------------------------------
// Step options (shared optional fields for all step types)
// ---------------------------------------------------------------------------

export interface StepOptions {
  /** Step IDs this step depends on (DAG edges). */
  depends_on?: Array<string | StepRef>;
  /** Optional JEXL expression for conditional execution. */
  condition?: string;
  /** Step-level timeout in seconds. */
  timeout_secs?: number;
  /** Retry configuration. */
  retry?: RetryConfig;
  /** Visual builder metadata. */
  ui?: StepUiMetadata;
}

// ---------------------------------------------------------------------------
// WorkflowBuilder
// ---------------------------------------------------------------------------

export class WorkflowBuilder {
  private _id: string;
  private _name: string;
  private _description?: string;
  private _version: string = "1.0.0";
  private _owner: WorkflowOwner = { type: "global" };
  private _concurrency?: number;
  private _timeout_secs?: number;
  private _triggers: TriggerConfig[] = [];
  private _steps: StepDefinition[] = [];
  private _metadata: Record<string, unknown> = {};
  private _stepIds: Set<string> = new Set();

  constructor(name: string, id?: string) {
    this._name = name;
    this._id = id ?? crypto.randomUUID();
  }

  /** Set the workflow ID (UUIDv7). */
  id(id: string): this {
    this._id = id;
    return this;
  }

  /** Set a longer description. */
  description(desc: string): this {
    this._description = desc;
    return this;
  }

  /** Set the semantic version (default "1.0.0"). */
  version(v: string): this {
    this._version = v;
    return this;
  }

  /** Set the workflow owner. */
  owner(o: WorkflowOwner): this {
    this._owner = o;
    return this;
  }

  /** Set maximum concurrent instances. */
  concurrency(n: number): this {
    this._concurrency = n;
    return this;
  }

  /** Set per-workflow timeout in seconds. */
  timeout(secs: number): this {
    this._timeout_secs = secs;
    return this;
  }

  /** Add a trigger configuration. */
  trigger(t: TriggerConfig): this {
    this._triggers.push(t);
    return this;
  }

  /** Add arbitrary metadata. */
  meta(key: string, value: unknown): this {
    this._metadata[key] = value;
    return this;
  }

  // -------------------------------------------------------------------------
  // Step factory methods
  // -------------------------------------------------------------------------

  /**
   * Add an agent step that invokes a bot with a prompt.
   * Returns a StepRef for type-safe dependency tracking.
   */
  agent(
    id: string,
    name: string,
    config: { bot: string; prompt: string; model?: string },
    opts?: StepOptions,
  ): StepRef {
    return this._addStep(id, name, "agent", { type: "agent", ...config }, opts);
  }

  /**
   * Add a skill step that runs an installed skill.
   */
  skill(
    id: string,
    name: string,
    config: { skill: string; input?: string },
    opts?: StepOptions,
  ): StepRef {
    return this._addStep(id, name, "skill", { type: "skill", ...config }, opts);
  }

  /**
   * Add a code step that executes inline TypeScript or WASM.
   */
  code(
    id: string,
    name: string,
    config: { language: CodeLanguage; source: string },
    opts?: StepOptions,
  ): StepRef {
    return this._addStep(id, name, "code", { type: "code", ...config }, opts);
  }

  /**
   * Add an HTTP request step.
   */
  http(
    id: string,
    name: string,
    config: {
      method: string;
      url: string;
      headers?: Record<string, string>;
      body?: string;
    },
    opts?: StepOptions,
  ): StepRef {
    return this._addStep(id, name, "http", { type: "http", ...config }, opts);
  }

  /**
   * Add a conditional branching step.
   */
  conditional(
    id: string,
    name: string,
    config: {
      condition: string;
      then_steps: Array<string | StepRef>;
      else_steps: Array<string | StepRef>;
    },
    opts?: StepOptions,
  ): StepRef {
    return this._addStep(
      id,
      name,
      "conditional",
      {
        type: "conditional",
        condition: config.condition,
        then_steps: config.then_steps.map(String),
        else_steps: config.else_steps.map(String),
      },
      opts,
    );
  }

  /**
   * Add a loop step.
   */
  loop(
    id: string,
    name: string,
    config: {
      condition: string;
      max_iterations?: number;
      body_steps: Array<string | StepRef>;
    },
    opts?: StepOptions,
  ): StepRef {
    return this._addStep(
      id,
      name,
      "loop",
      {
        type: "loop",
        condition: config.condition,
        max_iterations: config.max_iterations,
        body_steps: config.body_steps.map(String),
      },
      opts,
    );
  }

  /**
   * Add a human approval gate.
   */
  approval(
    id: string,
    name: string,
    config: { prompt: string; timeout_secs?: number },
    opts?: StepOptions,
  ): StepRef {
    return this._addStep(
      id,
      name,
      "approval",
      { type: "approval", ...config },
      opts,
    );
  }

  /**
   * Add a sub-workflow invocation step.
   */
  subWorkflow(
    id: string,
    name: string,
    config: { workflow_name: string; input?: unknown },
    opts?: StepOptions,
  ): StepRef {
    return this._addStep(
      id,
      name,
      "sub_workflow",
      { type: "sub_workflow", ...config },
      opts,
    );
  }

  // -------------------------------------------------------------------------
  // Build and export
  // -------------------------------------------------------------------------

  /**
   * Validate the workflow DAG and return the final WorkflowDefinition.
   * Throws if the workflow has invalid dependencies or duplicate step IDs.
   */
  build(): WorkflowDefinition {
    this._validateDag();

    const def: WorkflowDefinition = {
      id: this._id,
      name: this._name,
      version: this._version,
      owner: this._owner,
      triggers: this._triggers,
      steps: this._steps,
    };

    if (this._description !== undefined) def.description = this._description;
    if (this._concurrency !== undefined) def.concurrency = this._concurrency;
    if (this._timeout_secs !== undefined) def.timeout_secs = this._timeout_secs;
    if (Object.keys(this._metadata).length > 0) def.metadata = this._metadata;

    return def;
  }

  /**
   * Build the workflow and serialize it to YAML.
   */
  toYaml(): string {
    const def = this.build();
    return stringify(def, { lineWidth: 120 });
  }

  // -------------------------------------------------------------------------
  // Internal helpers
  // -------------------------------------------------------------------------

  private _addStep(
    id: string,
    name: string,
    stepType: StepType,
    config: StepConfig,
    opts?: StepOptions,
  ): StepRef {
    if (this._stepIds.has(id)) {
      throw new Error(`Duplicate step ID: "${id}"`);
    }
    this._stepIds.add(id);

    const step: StepDefinition = {
      id,
      name,
      type: stepType,
      config,
    };

    if (opts?.depends_on && opts.depends_on.length > 0) {
      step.depends_on = opts.depends_on.map(String);
    }
    if (opts?.condition !== undefined) step.condition = opts.condition;
    if (opts?.timeout_secs !== undefined) step.timeout_secs = opts.timeout_secs;
    if (opts?.retry !== undefined) step.retry = opts.retry;
    if (opts?.ui !== undefined) step.ui = opts.ui;

    this._steps.push(step);
    return new StepRef(id);
  }

  /**
   * Validate the DAG: ensure all depends_on references point to existing steps,
   * and detect cycles using topological sort (Kahn's algorithm).
   */
  private _validateDag(): void {
    const ids = new Set(this._steps.map((s) => s.id));

    // Check all dependencies reference existing steps
    for (const step of this._steps) {
      for (const dep of step.depends_on ?? []) {
        if (!ids.has(dep)) {
          throw new Error(
            `Step "${step.id}" depends on unknown step "${dep}"`,
          );
        }
      }
    }

    // Cycle detection via Kahn's algorithm
    const inDegree = new Map<string, number>();
    const adjacency = new Map<string, string[]>();

    for (const step of this._steps) {
      inDegree.set(step.id, 0);
      adjacency.set(step.id, []);
    }

    for (const step of this._steps) {
      for (const dep of step.depends_on ?? []) {
        adjacency.get(dep)!.push(step.id);
        inDegree.set(step.id, (inDegree.get(step.id) ?? 0) + 1);
      }
    }

    const queue: string[] = [];
    for (const [id, degree] of inDegree) {
      if (degree === 0) queue.push(id);
    }

    let visited = 0;
    while (queue.length > 0) {
      const current = queue.shift()!;
      visited++;
      for (const neighbor of adjacency.get(current) ?? []) {
        const newDegree = (inDegree.get(neighbor) ?? 1) - 1;
        inDegree.set(neighbor, newDegree);
        if (newDegree === 0) queue.push(neighbor);
      }
    }

    if (visited !== this._steps.length) {
      throw new Error(
        "Workflow DAG contains a cycle. Ensure step dependencies form an acyclic graph.",
      );
    }
  }
}

// ---------------------------------------------------------------------------
// Entry point: workflow() factory function
// ---------------------------------------------------------------------------

/**
 * Create a new workflow builder with the given name.
 *
 * ```ts
 * const wf = workflow("daily-digest")
 *   .version("1.0.0")
 *   .agent("gather", "Gather News", { bot: "researcher", prompt: "Find news" })
 *   .build();
 * ```
 */
export function workflow(name: string, id?: string): WorkflowBuilder {
  return new WorkflowBuilder(name, id);
}
