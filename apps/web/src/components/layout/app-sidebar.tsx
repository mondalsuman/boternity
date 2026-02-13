import { useQuery } from "@tanstack/react-query";
import { Link, useRouterState } from "@tanstack/react-router";
import {
  Bot,
  LayoutDashboard,
  MessageCircle,
  Settings,
} from "lucide-react";
import { apiFetch } from "@/lib/api-client";
import type { Bot as BotType } from "@/types/bot";
import {
  Sidebar,
  SidebarContent,
  SidebarGroup,
  SidebarGroupContent,
  SidebarGroupLabel,
  SidebarHeader,
  SidebarMenu,
  SidebarMenuButton,
  SidebarMenuItem,
  SidebarMenuSkeleton,
  SidebarMenuSub,
  SidebarMenuSubButton,
  SidebarMenuSubItem,
  SidebarRail,
  SidebarSeparator,
  SidebarTrigger,
} from "@/components/ui/sidebar";

/**
 * Main app sidebar with 4 navigation sections:
 * Dashboard, Bots (with last 5 bots inline), Chat, Settings.
 * Collapses to icon-only rail (VS Code style).
 */
export function AppSidebar() {
  const routerState = useRouterState();
  const currentPath = routerState.location.pathname;

  // Fetch last 5 bots for inline sidebar display
  const { data: bots, isLoading: botsLoading } = useQuery({
    queryKey: ["bots", { limit: 5, sort: "updated_at" }],
    queryFn: () =>
      apiFetch<BotType[]>("/bots?limit=5&sort=updated_at"),
    staleTime: 30_000, // 30s for sidebar
    retry: 1,
  });

  return (
    <Sidebar collapsible="icon">
      <SidebarHeader>
        <SidebarMenu>
          <SidebarMenuItem>
            <SidebarMenuButton size="lg" asChild>
              <Link to="/">
                <div className="bg-sidebar-primary text-sidebar-primary-foreground flex aspect-square size-8 items-center justify-center rounded-lg">
                  <Bot className="size-4" />
                </div>
                <div className="flex flex-col gap-0.5 leading-none">
                  <span className="font-semibold">Boternity</span>
                  <span className="text-xs text-muted-foreground">
                    Fleet Manager
                  </span>
                </div>
              </Link>
            </SidebarMenuButton>
          </SidebarMenuItem>
        </SidebarMenu>
        <SidebarTrigger className="ml-auto hidden md:inline-flex" />
      </SidebarHeader>

      <SidebarSeparator />

      <SidebarContent>
        {/* Dashboard */}
        <SidebarGroup>
          <SidebarGroupLabel>Navigation</SidebarGroupLabel>
          <SidebarGroupContent>
            <SidebarMenu>
              <SidebarMenuItem>
                <SidebarMenuButton
                  asChild
                  tooltip="Dashboard"
                  isActive={currentPath === "/"}
                >
                  <Link to="/">
                    <LayoutDashboard />
                    <span>Dashboard</span>
                  </Link>
                </SidebarMenuButton>
              </SidebarMenuItem>
            </SidebarMenu>
          </SidebarGroupContent>
        </SidebarGroup>

        {/* Bots with inline last 5 */}
        <SidebarGroup>
          <SidebarGroupLabel>Bots</SidebarGroupLabel>
          <SidebarGroupContent>
            <SidebarMenu>
              <SidebarMenuItem>
                <SidebarMenuButton
                  asChild
                  tooltip="Bots"
                  isActive={currentPath.startsWith("/bots")}
                >
                  <Link to="/">
                    <Bot />
                    <span>All Bots</span>
                  </Link>
                </SidebarMenuButton>
                {/* Last 5 bots inline */}
                <SidebarMenuSub>
                  {botsLoading ? (
                    <>
                      <SidebarMenuSkeleton />
                      <SidebarMenuSkeleton />
                      <SidebarMenuSkeleton />
                    </>
                  ) : (
                    bots?.map((bot) => (
                      <SidebarMenuSubItem key={bot.id}>
                        <SidebarMenuSubButton
                          asChild
                          isActive={currentPath === `/bots/${bot.id}`}
                        >
                          <Link to="/bots/$botId" params={{ botId: bot.id }}>
                            <span>{bot.emoji || bot.name.charAt(0)}</span>
                            <span className="truncate">{bot.name}</span>
                          </Link>
                        </SidebarMenuSubButton>
                      </SidebarMenuSubItem>
                    ))
                  )}
                </SidebarMenuSub>
              </SidebarMenuItem>
            </SidebarMenu>
          </SidebarGroupContent>
        </SidebarGroup>

        {/* Chat */}
        <SidebarGroup>
          <SidebarGroupLabel>Chat</SidebarGroupLabel>
          <SidebarGroupContent>
            <SidebarMenu>
              <SidebarMenuItem>
                <SidebarMenuButton
                  asChild
                  tooltip="Chat"
                  isActive={currentPath.startsWith("/chat")}
                >
                  <Link to="/chat" search={{ bot: undefined }}>
                    <MessageCircle />
                    <span>Chat</span>
                  </Link>
                </SidebarMenuButton>
              </SidebarMenuItem>
            </SidebarMenu>
          </SidebarGroupContent>
        </SidebarGroup>

        {/* Settings */}
        <SidebarGroup>
          <SidebarGroupLabel>Settings</SidebarGroupLabel>
          <SidebarGroupContent>
            <SidebarMenu>
              <SidebarMenuItem>
                <SidebarMenuButton
                  asChild
                  tooltip="Settings"
                  isActive={currentPath === "/settings"}
                >
                  <Link to="/settings">
                    <Settings />
                    <span>Settings</span>
                  </Link>
                </SidebarMenuButton>
              </SidebarMenuItem>
            </SidebarMenu>
          </SidebarGroupContent>
        </SidebarGroup>
      </SidebarContent>

      <SidebarRail />
    </Sidebar>
  );
}
