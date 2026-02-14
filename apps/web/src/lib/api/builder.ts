/**
 * Builder API client for the bot creation wizard.
 *
 * Typed functions wrapping the REST endpoints from the builder system.
 * All calls use the shared apiFetch wrapper for envelope unwrapping,
 * auth headers, and error handling.
 */

import { apiFetch } from "@/lib/api-client";

// ---------------------------------------------------------------------------
// Types mirroring Rust builder domain (crates/boternity-types/src/builder.rs)
// ---------------------------------------------------------------------------

/** Phase of the builder wizard flow. */
export type BuilderPhase =
  | "basics"
  | "personality"
  | "model"
  | "skills"
  | "review";

/** Phase display labels for the step indicator. */
export const PHASE_LABELS: Record<BuilderPhase, string> = {
  basics: "Basics",
  personality: "Personality",
  model: "Model",
  skills: "Skills",
  review: "Review",
};

/** Ordered list of phases for step indicator index calculation. */
export const PHASE_ORDER: BuilderPhase[] = [
  "basics",
  "personality",
  "model",
  "skills",
  "review",
];

/** A single option in a builder question. */
export interface QuestionOption {
  id: string;
  label: string;
  description?: string;
}

/** Preview of the bot configuration as it is being assembled. */
export interface BuilderPreview {
  name?: string;
  description?: string;
  personality_summary?: string;
  model?: string;
  skills: string[];
  phase: BuilderPhase;
}

/** Personality configuration for a bot. */
export interface PersonalityConfig {
  tone: string;
  traits: string[];
  purpose: string;
  boundaries?: string;
}

/** Model configuration for a bot. */
export interface ModelConfig {
  model: string;
  temperature: number;
  max_tokens: number;
}

/** A skill to be attached to the bot. */
export interface SkillRequest {
  name: string;
  description: string;
  skill_type: string;
}

/** The fully assembled builder configuration. */
export interface BuilderConfig {
  name: string;
  description: string;
  category: string;
  tags: string[];
  personality: PersonalityConfig;
  model_config: ModelConfig;
  skills: SkillRequest[];
}

/**
 * A single turn produced by the builder LLM.
 *
 * Tagged union on the `action` field, matching the Rust serde representation.
 */
export type BuilderTurn =
  | {
      action: "ask_question";
      phase: BuilderPhase;
      question: string;
      options: QuestionOption[];
      allow_free_text: boolean;
      phase_label?: string;
    }
  | {
      action: "show_preview";
      phase: BuilderPhase;
      preview: BuilderPreview;
    }
  | {
      action: "ready_to_assemble";
      config: BuilderConfig;
    }
  | {
      action: "clarify";
      message: string;
    };

/** State summary returned alongside each turn. */
export interface StateSummary {
  phase: BuilderPhase;
  question_count: number;
}

/**
 * User answer to a builder question.
 *
 * Tagged union matching Rust BuilderAnswer serde representation.
 */
export type BuilderAnswer =
  | { OptionIndex: number }
  | { FreeText: string }
  | { Confirm: boolean }
  | "Back";

/** Assembly result after bot creation. */
export interface AssemblyResult {
  bot_id: string;
  bot_slug: string;
  bot_name: string;
  soul_path: string;
  identity_path: string;
  user_path: string;
  skills_attached: string[];
}

/** Draft summary for the resume-draft listing. */
export interface DraftSummary {
  session_id: string;
  initial_description: string;
  phase: string;
  updated_at: string;
}

// ---------------------------------------------------------------------------
// Response shapes matching the Rust handler DTOs
// ---------------------------------------------------------------------------

interface CreateSessionResponse {
  session_id: string;
  mode: string;
  turn: BuilderTurn;
}

interface SubmitAnswerResponse {
  turn: BuilderTurn;
  state_summary: StateSummary;
}

interface AssembleBotResponse {
  result: AssemblyResult;
}

interface GetSessionResponse {
  session_id: string;
  phase: string;
  initial_description: string;
  question_count: number;
}

// ---------------------------------------------------------------------------
// API functions
// ---------------------------------------------------------------------------

/**
 * Create a new builder session.
 *
 * Starts the wizard conversation and returns the first turn.
 */
export async function createBuilderSession(
  description: string,
  mode: string = "bot",
): Promise<CreateSessionResponse> {
  return apiFetch<CreateSessionResponse>("/builder/sessions", {
    method: "POST",
    body: JSON.stringify({ description, mode }),
  });
}

/**
 * Submit an answer to the current builder question.
 *
 * Advances the conversation and returns the next turn.
 */
export async function submitAnswer(
  sessionId: string,
  answer: BuilderAnswer,
): Promise<SubmitAnswerResponse> {
  return apiFetch<SubmitAnswerResponse>(
    `/builder/sessions/${sessionId}/answer`,
    {
      method: "POST",
      body: JSON.stringify({ answer }),
    },
  );
}

/**
 * Assemble a bot from the finalized configuration.
 *
 * Creates the bot, writes files to disk, and deletes the draft.
 */
export async function assembleBot(
  sessionId: string,
  config: BuilderConfig,
): Promise<AssemblyResult> {
  const resp = await apiFetch<AssembleBotResponse>(
    `/builder/sessions/${sessionId}/assemble`,
    {
      method: "POST",
      body: JSON.stringify({ config }),
    },
  );
  return resp.result;
}

/**
 * Get session state summary.
 */
export async function getSession(
  sessionId: string,
): Promise<GetSessionResponse> {
  return apiFetch<GetSessionResponse>(`/builder/sessions/${sessionId}`);
}

/**
 * List all saved builder drafts.
 */
export async function listDrafts(): Promise<DraftSummary[]> {
  return apiFetch<DraftSummary[]>("/builder/drafts");
}

/**
 * Delete a builder session / draft.
 */
export async function deleteSession(sessionId: string): Promise<void> {
  await apiFetch<unknown>(`/builder/sessions/${sessionId}`, {
    method: "DELETE",
  });
}
