import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { apiFetch } from "@/lib/api-client";
import type {
  InstalledSkill,
  BotSkillConfig,
  DiscoveredSkill,
} from "@/types/skill";
import { toast } from "sonner";

/** Fetch all globally installed skills. */
export function useAllSkills() {
  return useQuery({
    queryKey: ["skills"],
    queryFn: () => apiFetch<InstalledSkill[]>("/skills"),
    staleTime: 30_000,
  });
}

/** Fetch skills attached to a specific bot. */
export function useBotSkills(botId: string) {
  return useQuery({
    queryKey: ["bot", botId, "skills"],
    queryFn: () => apiFetch<BotSkillConfig[]>(`/bots/${botId}/skills`),
    enabled: !!botId,
  });
}

/** Attach a skill to a bot. */
export function useAttachSkill() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: ({
      botId,
      skillName,
    }: {
      botId: string;
      skillName: string;
    }) =>
      apiFetch<unknown>(`/bots/${botId}/skills`, {
        method: "POST",
        body: JSON.stringify({ skill_name: skillName }),
      }),
    onSuccess: (_data, variables) => {
      queryClient.invalidateQueries({
        queryKey: ["bot", variables.botId, "skills"],
      });
      toast.success("Skill attached");
    },
    onError: (error) => {
      toast.error(error.message || "Failed to attach skill");
    },
  });
}

/** Detach a skill from a bot. */
export function useDetachSkill() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: ({
      botId,
      skillName,
    }: {
      botId: string;
      skillName: string;
    }) =>
      apiFetch<void>(`/bots/${botId}/skills/${skillName}`, {
        method: "DELETE",
      }),
    onSuccess: (_data, variables) => {
      queryClient.invalidateQueries({
        queryKey: ["bot", variables.botId, "skills"],
      });
      toast.success("Skill detached");
    },
    onError: (error) => {
      toast.error(error.message || "Failed to detach skill");
    },
  });
}

/** Toggle a skill's enabled state on a bot. */
export function useToggleSkill() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: ({
      botId,
      skillName,
      enabled,
    }: {
      botId: string;
      skillName: string;
      enabled: boolean;
    }) =>
      apiFetch<unknown>(`/bots/${botId}/skills/${skillName}`, {
        method: "PATCH",
        body: JSON.stringify({ enabled }),
      }),
    onSuccess: (_data, variables) => {
      queryClient.invalidateQueries({
        queryKey: ["bot", variables.botId, "skills"],
      });
    },
    onError: (error) => {
      toast.error(error.message || "Failed to update skill");
    },
  });
}

/** Search registries for skills. */
export function useRegistrySearch(query: string) {
  return useQuery({
    queryKey: ["registry", "search", query],
    queryFn: () =>
      apiFetch<DiscoveredSkill[]>(
        `/registry/search?q=${encodeURIComponent(query)}`,
      ),
    enabled: query.length >= 2,
    staleTime: 60_000,
  });
}

/** Install a skill from a registry. */
export function useInstallSkill() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: ({
      source,
      skillName,
    }: {
      source: string;
      skillName?: string;
    }) =>
      apiFetch<unknown>("/skills/install", {
        method: "POST",
        body: JSON.stringify({
          source,
          skill_name: skillName,
          capabilities_approved: [],
        }),
      }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["skills"] });
      toast.success("Skill installed");
    },
    onError: (error) => {
      toast.error(error.message || "Failed to install skill");
    },
  });
}
