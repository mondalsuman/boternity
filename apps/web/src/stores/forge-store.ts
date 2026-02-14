/**
 * Zustand store for Forge chat conversation state.
 *
 * Manages the message history, preview state, and assembly/skill creation
 * results for the Forge bot builder chat interface. Supports both bot
 * creation and standalone skill creation modes.
 *
 * Each BuilderTurn from the WebSocket is converted to a ForgeMessage
 * for display in the chat UI. Interactive turns (AskQuestion) carry
 * the full turn data so the UI can render option buttons.
 */

import { create } from "zustand";
import type {
  BuilderTurn,
  BuilderPreview,
  AssemblyResult,
} from "@/lib/api/builder";

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/** Whether the Forge session is creating a bot or a standalone skill. */
export type ForgeMode = "bot" | "skill" | null;

/** A single message in the Forge chat conversation. */
export interface ForgeMessage {
  id: string;
  role: "forge" | "user";
  content: string;
  turn?: BuilderTurn;
  timestamp: Date;
}

/** Skill creation result from the WebSocket. */
export interface SkillCreatedResult {
  name: string;
  description: string;
  has_source_code: boolean;
  suggested_capabilities: string[];
}

// ---------------------------------------------------------------------------
// Store interface
// ---------------------------------------------------------------------------

interface ForgeState {
  sessionId: string | null;
  mode: ForgeMode;
  messages: ForgeMessage[];
  preview: BuilderPreview | null;
  isAssembled: boolean;
  assemblyResult: AssemblyResult | null;
  skillResult: SkillCreatedResult | null;
  isWaiting: boolean;

  // Actions
  setSessionId: (id: string) => void;
  setMode: (mode: ForgeMode) => void;
  addForgeMessage: (turn: BuilderTurn) => void;
  addUserMessage: (text: string) => void;
  setPreview: (preview: BuilderPreview) => void;
  setAssembled: (result: AssemblyResult) => void;
  setSkillCreated: (result: SkillCreatedResult) => void;
  setWaiting: (waiting: boolean) => void;
  reset: () => void;
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

let messageCounter = 0;

function nextId(): string {
  messageCounter += 1;
  return `forge-msg-${Date.now()}-${messageCounter}`;
}

// ---------------------------------------------------------------------------
// Store
// ---------------------------------------------------------------------------

const initialState = {
  sessionId: null as string | null,
  mode: null as ForgeMode,
  messages: [] as ForgeMessage[],
  preview: null as BuilderPreview | null,
  isAssembled: false,
  assemblyResult: null as AssemblyResult | null,
  skillResult: null as SkillCreatedResult | null,
  isWaiting: false,
};

export const useForgeStore = create<ForgeState>()((set, get) => ({
  ...initialState,

  setSessionId: (id: string) => set({ sessionId: id }),

  setMode: (mode: ForgeMode) => set({ mode }),

  addForgeMessage: (turn: BuilderTurn) => {
    const { mode } = get();

    let content: string;
    let updatedPreview: BuilderPreview | null = null;

    switch (turn.action) {
      case "ask_question":
        content = turn.question;
        break;
      case "show_preview":
        content = "Here's what we have so far...";
        updatedPreview = turn.preview;
        break;
      case "ready_to_assemble":
        content =
          mode === "skill"
            ? "I've got everything I need! Here's your skill:"
            : "I've got everything I need! Here's your bot:";
        break;
      case "clarify":
        content = turn.message;
        break;
      default:
        content = "...";
    }

    const message: ForgeMessage = {
      id: nextId(),
      role: "forge",
      content,
      turn,
      timestamp: new Date(),
    };

    set((state) => ({
      messages: [...state.messages, message],
      isWaiting: false,
      ...(updatedPreview ? { preview: updatedPreview } : {}),
    }));
  },

  addUserMessage: (text: string) => {
    const message: ForgeMessage = {
      id: nextId(),
      role: "user",
      content: text,
      timestamp: new Date(),
    };

    set((state) => ({
      messages: [...state.messages, message],
      isWaiting: true,
    }));
  },

  setPreview: (preview: BuilderPreview) => set({ preview }),

  setAssembled: (result: AssemblyResult) =>
    set({ isAssembled: true, assemblyResult: result, isWaiting: false }),

  setSkillCreated: (result: SkillCreatedResult) =>
    set({ skillResult: result, isAssembled: true, isWaiting: false }),

  setWaiting: (waiting: boolean) => set({ isWaiting: waiting }),

  reset: () => {
    set(initialState);
  },
}));
