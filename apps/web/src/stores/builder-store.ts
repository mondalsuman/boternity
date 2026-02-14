/**
 * Builder wizard state store (Zustand).
 *
 * Manages the builder conversation flow: session creation, answer submission,
 * back navigation via turn history, and bot assembly. Server data drives
 * the wizard -- the store tracks the current turn and accumulated history.
 */

import { create } from "zustand";

import type {
  BuilderAnswer,
  BuilderConfig,
  BuilderTurn,
  BuilderPreview,
  AssemblyResult,
  BuilderPhase,
} from "@/lib/api/builder";
import {
  createBuilderSession,
  submitAnswer as apiSubmitAnswer,
  assembleBot,
} from "@/lib/api/builder";

interface BuilderState {
  /** Active builder session ID. */
  sessionId: string | null;
  /** Current turn from the builder LLM. */
  currentTurn: BuilderTurn | null;
  /** Current wizard phase (from the latest turn or state summary). */
  phase: BuilderPhase;
  /** History of previous turns for back navigation. */
  history: BuilderTurn[];
  /** Latest preview snapshot (accumulated from ShowPreview turns). */
  preview: BuilderPreview | null;
  /** Loading state for API calls. */
  isLoading: boolean;
  /** Error message from the last failed API call. */
  error: string | null;

  // Actions
  /** Start a new builder session with the given description. */
  startSession: (description: string) => Promise<void>;
  /** Submit an answer to the current question. */
  submitAnswer: (answer: BuilderAnswer) => Promise<void>;
  /** Go back to the previous step. */
  goBack: () => Promise<void>;
  /** Assemble the bot from the finalized configuration. */
  assemble: (config: BuilderConfig) => Promise<AssemblyResult>;
  /** Reset the store to initial state. */
  reset: () => void;
}

const initialState = {
  sessionId: null,
  currentTurn: null,
  phase: "basics" as BuilderPhase,
  history: [],
  preview: null,
  isLoading: false,
  error: null,
};

/**
 * Extract the phase from a BuilderTurn.
 */
function phaseFromTurn(turn: BuilderTurn): BuilderPhase {
  switch (turn.action) {
    case "ask_question":
      return turn.phase;
    case "show_preview":
      return turn.phase;
    case "ready_to_assemble":
      return "review";
    case "clarify":
      return "basics"; // clarify stays in current context
  }
}

/**
 * Extract preview from a BuilderTurn if it contains one.
 */
function previewFromTurn(
  turn: BuilderTurn,
  existing: BuilderPreview | null,
): BuilderPreview | null {
  if (turn.action === "show_preview") {
    return turn.preview;
  }
  return existing;
}

export const useBuilderStore = create<BuilderState>()((set, get) => ({
  ...initialState,

  startSession: async (description: string) => {
    set({ isLoading: true, error: null });
    try {
      const resp = await createBuilderSession(description);
      const turn = resp.turn;
      set({
        sessionId: resp.session_id,
        currentTurn: turn,
        phase: phaseFromTurn(turn),
        preview: previewFromTurn(turn, null),
        history: [],
        isLoading: false,
      });
    } catch (err) {
      const message =
        err instanceof Error ? err.message : "Failed to start builder session";
      set({ error: message, isLoading: false });
    }
  },

  submitAnswer: async (answer: BuilderAnswer) => {
    const { sessionId, currentTurn } = get();
    if (!sessionId || !currentTurn) return;

    set({ isLoading: true, error: null });
    try {
      const resp = await apiSubmitAnswer(sessionId, answer);
      const turn = resp.turn;
      const newPhase = resp.state_summary?.phase ?? phaseFromTurn(turn);
      set((state) => ({
        currentTurn: turn,
        phase: newPhase,
        preview: previewFromTurn(turn, state.preview),
        // Push previous turn to history for back navigation
        history: [...state.history, currentTurn],
        isLoading: false,
      }));
    } catch (err) {
      const message =
        err instanceof Error ? err.message : "Failed to submit answer";
      set({ error: message, isLoading: false });
    }
  },

  goBack: async () => {
    const { sessionId } = get();
    if (!sessionId) return;

    set({ isLoading: true, error: null });
    try {
      const resp = await apiSubmitAnswer(sessionId, "Back");
      const turn = resp.turn;
      const newPhase = resp.state_summary?.phase ?? phaseFromTurn(turn);
      set((state) => ({
        currentTurn: turn,
        phase: newPhase,
        preview: previewFromTurn(turn, state.preview),
        // Pop the last turn from history
        history: state.history.slice(0, -1),
        isLoading: false,
      }));
    } catch (err) {
      const message =
        err instanceof Error ? err.message : "Failed to go back";
      set({ error: message, isLoading: false });
    }
  },

  assemble: async (config: BuilderConfig) => {
    const { sessionId } = get();
    if (!sessionId) {
      throw new Error("No active builder session");
    }

    set({ isLoading: true, error: null });
    try {
      const result = await assembleBot(sessionId, config);
      set({ isLoading: false });
      return result;
    } catch (err) {
      const message =
        err instanceof Error ? err.message : "Failed to assemble bot";
      set({ error: message, isLoading: false });
      throw err;
    }
  },

  reset: () => set(initialState),
}));
