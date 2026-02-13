import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { apiFetch } from "@/lib/api-client";
import type { Bot, CreateBotRequest, UpdateBotRequest } from "@/types/bot";
import { toast } from "sonner";

export interface BotFilter {
  status?: string;
  sort?: string;
  search?: string;
  limit?: number;
  offset?: number;
}

function buildQueryString(filter?: BotFilter): string {
  if (!filter) return "";
  const params = new URLSearchParams();
  if (filter.status) params.set("status", filter.status);
  if (filter.sort) params.set("sort", filter.sort);
  if (filter.search) params.set("search", filter.search);
  if (filter.limit != null) params.set("limit", String(filter.limit));
  if (filter.offset != null) params.set("offset", String(filter.offset));
  const qs = params.toString();
  return qs ? `?${qs}` : "";
}

export function useBots(filter?: BotFilter) {
  return useQuery({
    queryKey: ["bots", filter],
    queryFn: () => apiFetch<Bot[]>(`/bots${buildQueryString(filter)}`),
    staleTime: 10_000,
    placeholderData: (prev) => prev,
  });
}

export function useBot(idOrSlug: string) {
  return useQuery({
    queryKey: ["bot", idOrSlug],
    queryFn: () => apiFetch<Bot>(`/bots/${idOrSlug}`),
    enabled: !!idOrSlug,
  });
}

export function useCreateBot() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: (data: CreateBotRequest) =>
      apiFetch<Bot>("/bots", {
        method: "POST",
        body: JSON.stringify(data),
      }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["bots"] });
      queryClient.invalidateQueries({ queryKey: ["stats"] });
      toast.success("Bot created");
    },
    onError: (error) => {
      toast.error(error.message || "Failed to create bot");
    },
  });
}

export function useUpdateBot() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: ({ id, data }: { id: string; data: UpdateBotRequest }) =>
      apiFetch<Bot>(`/bots/${id}`, {
        method: "PUT",
        body: JSON.stringify(data),
      }),
    onSuccess: (_data, variables) => {
      queryClient.invalidateQueries({ queryKey: ["bots"] });
      queryClient.invalidateQueries({ queryKey: ["bot", variables.id] });
    },
    onError: (error) => {
      toast.error(error.message || "Failed to update bot");
    },
  });
}

export function useDeleteBot() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: (id: string) =>
      apiFetch<void>(`/bots/${id}`, { method: "DELETE" }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["bots"] });
      queryClient.invalidateQueries({ queryKey: ["stats"] });
      toast.success("Bot deleted");
    },
    onError: (error) => {
      toast.error(error.message || "Failed to delete bot");
    },
  });
}
