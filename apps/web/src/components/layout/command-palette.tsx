import { useCallback, useEffect, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { useRouter } from "@tanstack/react-router";
import {
  Bot,
  LayoutDashboard,
  MessageCircle,
  Plus,
  Settings,
} from "lucide-react";
import { apiFetch } from "@/lib/api-client";
import type { Bot as BotType } from "@/types/bot";
import {
  CommandDialog,
  CommandEmpty,
  CommandGroup,
  CommandInput,
  CommandItem,
  CommandList,
  CommandSeparator,
} from "@/components/ui/command";

/**
 * Global command palette (Cmd+K / Ctrl+K).
 * Groups: Navigation, Bots (dynamic), Actions.
 */
export function CommandPalette() {
  const [open, setOpen] = useState(false);
  const router = useRouter();

  // Register Cmd+K / Ctrl+K keyboard shortcut
  useEffect(() => {
    function handleKeyDown(e: KeyboardEvent) {
      if (e.key === "k" && (e.metaKey || e.ctrlKey)) {
        e.preventDefault();
        setOpen((prev) => !prev);
      }
    }
    document.addEventListener("keydown", handleKeyDown);
    return () => document.removeEventListener("keydown", handleKeyDown);
  }, []);

  // Fetch bots when palette is open
  const { data: bots } = useQuery({
    queryKey: ["bots"],
    queryFn: () => apiFetch<BotType[]>("/bots"),
    enabled: open,
    staleTime: 10_000,
  });

  const navigate = useCallback(
    (to: string) => {
      router.navigate({ to });
      setOpen(false);
    },
    [router],
  );

  return (
    <CommandDialog open={open} onOpenChange={setOpen}>
      <CommandInput placeholder="Type a command or search..." />
      <CommandList>
        <CommandEmpty>No results found.</CommandEmpty>

        {/* Navigation */}
        <CommandGroup heading="Navigation">
          <CommandItem onSelect={() => navigate("/")}>
            <LayoutDashboard className="mr-2 h-4 w-4" />
            <span>Dashboard</span>
          </CommandItem>
          <CommandItem onSelect={() => navigate("/chat")}>
            <MessageCircle className="mr-2 h-4 w-4" />
            <span>Chat</span>
          </CommandItem>
          <CommandItem onSelect={() => navigate("/settings")}>
            <Settings className="mr-2 h-4 w-4" />
            <span>Settings</span>
          </CommandItem>
        </CommandGroup>

        <CommandSeparator />

        {/* Bots */}
        {bots && bots.length > 0 && (
          <CommandGroup heading="Bots">
            {bots.map((bot) => (
              <CommandItem
                key={bot.id}
                onSelect={() => navigate(`/bots/${bot.id}`)}
              >
                <span className="mr-2">{bot.emoji || "🤖"}</span>
                <span>{bot.name}</span>
              </CommandItem>
            ))}
          </CommandGroup>
        )}

        <CommandSeparator />

        {/* Actions */}
        <CommandGroup heading="Actions">
          <CommandItem onSelect={() => navigate("/")}>
            <Plus className="mr-2 h-4 w-4" />
            <span>New Bot</span>
          </CommandItem>
          <CommandItem onSelect={() => navigate("/chat")}>
            <Plus className="mr-2 h-4 w-4" />
            <span>New Chat</span>
          </CommandItem>
        </CommandGroup>
      </CommandList>
    </CommandDialog>
  );
}
