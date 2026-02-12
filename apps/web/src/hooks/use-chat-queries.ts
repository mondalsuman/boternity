/**
 * TanStack Query hooks for chat sessions and messages.
 *
 * Manages session listing (per bot and all bots), message fetching,
 * and session CRUD mutations.
 */

import {
  useQuery,
  useQueries,
  useMutation,
  useQueryClient,
} from "@tanstack/react-query";
import { apiFetch } from "@/lib/api-client";
import type { ChatSession, ChatMessage } from "@/types/chat";
import type { Bot } from "@/types/bot";
import { toast } from "sonner";

/**
 * Fetch all sessions for a specific bot.
 */
export function useBotSessions(botId: string | null) {
  return useQuery({
    queryKey: ["sessions", botId],
    queryFn: () => apiFetch<ChatSession[]>(`/bots/${botId}/sessions`),
    enabled: !!botId,
    staleTime: 5_000,
  });
}

/**
 * Fetch a single session by ID.
 */
export function useSession(sessionId: string | null) {
  return useQuery({
    queryKey: ["session", sessionId],
    queryFn: () => apiFetch<ChatSession>(`/sessions/${sessionId}`),
    enabled: !!sessionId,
    staleTime: 5_000,
  });
}

/**
 * Grouped session data: sessions organized by bot.
 */
export interface BotSessionGroup {
  bot: Bot;
  sessions: ChatSession[];
}

/**
 * Fetch all sessions across all bots, grouped by bot.
 * First fetches the bot list, then sessions for each bot in parallel.
 */
export function useAllSessions() {
  const botsQuery = useQuery({
    queryKey: ["bots"],
    queryFn: () => apiFetch<Bot[]>("/bots"),
    staleTime: 10_000,
  });

  const bots = botsQuery.data ?? [];

  const sessionQueries = useQueries({
    queries: bots.map((bot) => ({
      queryKey: ["sessions", bot.id],
      queryFn: () => apiFetch<ChatSession[]>(`/bots/${bot.id}/sessions`),
      staleTime: 5_000,
      enabled: bots.length > 0,
    })),
  });

  const isLoading =
    botsQuery.isLoading || sessionQueries.some((q) => q.isLoading);
  const isError = botsQuery.isError || sessionQueries.some((q) => q.isError);

  // Build grouped data
  const groups: BotSessionGroup[] = bots
    .map((bot, i) => ({
      bot,
      sessions: sessionQueries[i]?.data ?? [],
    }))
    .filter((g) => g.sessions.length > 0);

  return { groups, bots, isLoading, isError };
}

/**
 * Fetch messages for a session.
 * staleTime: 0 -- always fresh during active chat.
 */
export function useMessages(sessionId: string | null) {
  return useQuery({
    queryKey: ["messages", sessionId],
    queryFn: () => apiFetch<ChatMessage[]>(`/sessions/${sessionId}/messages`),
    enabled: !!sessionId,
    staleTime: 0,
  });
}

/**
 * Delete a session and all its messages.
 */
export function useDeleteSession() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: (sessionId: string) =>
      apiFetch<void>(`/sessions/${sessionId}`, { method: "DELETE" }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["sessions"] });
      queryClient.invalidateQueries({ queryKey: ["stats"] });
      toast.success("Session deleted");
    },
    onError: (error) => {
      toast.error(error.message || "Failed to delete session");
    },
  });
}

/**
 * Clear all messages in a session but keep the session itself.
 */
export function useClearSession() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: (sessionId: string) =>
      apiFetch<void>(`/sessions/${sessionId}/clear`, { method: "POST" }),
    onSuccess: (_data, sessionId) => {
      queryClient.invalidateQueries({
        queryKey: ["messages", sessionId],
      });
      toast.success("Session cleared");
    },
    onError: (error) => {
      toast.error(error.message || "Failed to clear session");
    },
  });
}
