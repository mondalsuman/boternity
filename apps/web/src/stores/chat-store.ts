/**
 * Chat UI state store (Zustand).
 *
 * Tracks active bot and session selection for the chat interface.
 * Server data (sessions, messages) lives in TanStack Query, not here.
 */

import { create } from "zustand";

interface ChatStore {
  /** Which bot is active in the chat view */
  activeBotId: string | null;
  /** Which session is currently displayed */
  activeSessionId: string | null;
  /** Set the active bot for chat */
  setActiveBot: (botId: string | null) => void;
  /** Set the active session for display */
  setActiveSession: (sessionId: string | null) => void;
}

export const useChatStore = create<ChatStore>()((set) => ({
  activeBotId: null,
  activeSessionId: null,
  setActiveBot: (botId) => set({ activeBotId: botId }),
  setActiveSession: (sessionId) => set({ activeSessionId: sessionId }),
}));
