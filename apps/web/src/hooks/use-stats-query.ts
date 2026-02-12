import { useQuery } from "@tanstack/react-query";
import { apiFetch } from "@/lib/api-client";

export interface Stats {
  total_bots: number;
  active_bots: number;
  disabled_bots: number;
  archived_bots: number;
  total_sessions: number;
  active_sessions: number;
  total_messages: number;
}

export function useStats() {
  return useQuery({
    queryKey: ["stats"],
    queryFn: () => apiFetch<Stats>("/stats"),
    staleTime: 30_000,
  });
}
