import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { apiFetch } from "@/lib/api-client";
import type { Soul, SoulVersion } from "@/types/soul";
import { toast } from "sonner";

// ----- Soul (SOUL.md) -----

/** Fetch the current soul content for a bot. */
export function useSoul(botId: string) {
  return useQuery({
    queryKey: ["soul", botId],
    queryFn: () => apiFetch<Soul>(`/bots/${botId}/soul`),
    enabled: !!botId,
    staleTime: 30_000,
  });
}

/** Fetch all soul versions for a bot. */
export function useSoulVersions(botId: string) {
  return useQuery({
    queryKey: ["soul-versions", botId],
    queryFn: () => apiFetch<SoulVersion[]>(`/bots/${botId}/soul/versions`),
    enabled: !!botId,
    staleTime: 30_000,
  });
}

/** Update the soul content (creates a new version). */
export function useUpdateSoul(botId: string) {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: (data: { content: string; message?: string }) =>
      apiFetch<Soul>(`/bots/${botId}/soul`, {
        method: "PUT",
        body: JSON.stringify(data),
      }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["soul", botId] });
      queryClient.invalidateQueries({ queryKey: ["soul-versions", botId] });
      queryClient.invalidateQueries({ queryKey: ["bot", botId] });
      toast.success("Soul saved");
    },
    onError: (error) => {
      toast.error(error.message || "Failed to save soul");
    },
  });
}

// ----- Identity (IDENTITY.md) -----

/** API response shape for the identity endpoint. */
export interface IdentityResponse {
  raw: string;
  parsed: {
    display_name: string | null;
    category: string | null;
    model: string | null;
    provider: string | null;
    temperature: number | null;
    max_tokens: number | null;
  } | null;
}

/** Fetch the identity file content with parsed frontmatter. */
export function useIdentity(botId: string) {
  return useQuery({
    queryKey: ["identity", botId],
    queryFn: () => apiFetch<IdentityResponse>(`/bots/${botId}/identity`),
    enabled: !!botId,
    staleTime: 30_000,
  });
}

/** Update the identity file content. */
export function useUpdateIdentity(botId: string) {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: (content: string) =>
      apiFetch<IdentityResponse>(`/bots/${botId}/identity`, {
        method: "PUT",
        body: JSON.stringify({ content }),
      }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["identity", botId] });
      toast.success("Identity saved");
    },
    onError: (error) => {
      toast.error(error.message || "Failed to save identity");
    },
  });
}

// ----- User Context (USER.md) -----

/** API response shape for the user context endpoint. */
export interface UserContextResponse {
  content: string;
}

/** Fetch the user context file content. */
export function useUserContext(botId: string) {
  return useQuery({
    queryKey: ["user-context", botId],
    queryFn: () => apiFetch<UserContextResponse>(`/bots/${botId}/user`),
    enabled: !!botId,
    staleTime: 30_000,
  });
}

/** Update the user context file content. */
export function useUpdateUserContext(botId: string) {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: (content: string) =>
      apiFetch<UserContextResponse>(`/bots/${botId}/user`, {
        method: "PUT",
        body: JSON.stringify({ content }),
      }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["user-context", botId] });
      toast.success("User context saved");
    },
    onError: (error) => {
      toast.error(error.message || "Failed to save user context");
    },
  });
}
