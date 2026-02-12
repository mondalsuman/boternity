import { useState } from "react";
import { Link, useNavigate } from "@tanstack/react-router";
import { formatDistanceToNow } from "date-fns";
import {
  MessageSquare,
  MoreVertical,
  Pencil,
  Power,
  PowerOff,
  Trash2,
} from "lucide-react";
import type { Bot, BotStatus } from "@/types/bot";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardFooter,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from "@/components/ui/alert-dialog";
import { useDeleteBot, useUpdateBot } from "@/hooks/use-bot-queries";
import { cn } from "@/lib/utils";

export const STATUS_COLORS: Record<BotStatus, string> = {
  active: "bg-green-500",
  disabled: "bg-yellow-500",
  archived: "bg-red-500",
};

const STATUS_LABELS: Record<BotStatus, string> = {
  active: "Active",
  disabled: "Disabled",
  archived: "Archived",
};

interface BotCardProps {
  bot: Bot;
}

export function BotCard({ bot }: BotCardProps) {
  const navigate = useNavigate();
  const deleteMutation = useDeleteBot();
  const updateMutation = useUpdateBot();
  const [deleteOpen, setDeleteOpen] = useState(false);

  const emoji = bot.emoji || bot.name.charAt(0).toUpperCase();
  const lastActivity = bot.updated_at
    ? formatDistanceToNow(new Date(bot.updated_at), { addSuffix: true })
    : "Never";

  function handleToggleStatus() {
    const newStatus: BotStatus = bot.status === "active" ? "disabled" : "active";
    updateMutation.mutate({ id: bot.id, data: { status: newStatus } });
  }

  function handleDelete() {
    deleteMutation.mutate(bot.id, {
      onSuccess: () => setDeleteOpen(false),
    });
  }

  return (
    <>
      <Card className="group relative transition-colors hover:border-muted-foreground/30">
        <CardHeader className="pb-0">
          <div className="flex items-start justify-between">
            <div className="flex items-center gap-2.5 min-w-0">
              <span className="text-2xl shrink-0" role="img" aria-label={`${bot.name} avatar`}>
                {emoji}
              </span>
              <div className="min-w-0">
                <CardTitle className="truncate text-base">{bot.name}</CardTitle>
                <Badge variant="outline" className="mt-1 gap-1.5 text-[10px]">
                  <span
                    className={cn(
                      "inline-block size-1.5 rounded-full",
                      STATUS_COLORS[bot.status],
                    )}
                  />
                  {STATUS_LABELS[bot.status]}
                </Badge>
              </div>
            </div>

            <DropdownMenu>
              <DropdownMenuTrigger asChild>
                <Button
                  variant="ghost"
                  size="icon-xs"
                  className="opacity-0 group-hover:opacity-100 transition-opacity shrink-0"
                >
                  <MoreVertical className="size-4" />
                  <span className="sr-only">Actions</span>
                </Button>
              </DropdownMenuTrigger>
              <DropdownMenuContent align="end">
                <DropdownMenuItem
                  onClick={() =>
                    navigate({ to: "/bots/$botId", params: { botId: bot.id } })
                  }
                >
                  <Pencil />
                  Edit
                </DropdownMenuItem>
                <DropdownMenuItem onClick={handleToggleStatus}>
                  {bot.status === "active" ? (
                    <>
                      <PowerOff />
                      Disable
                    </>
                  ) : (
                    <>
                      <Power />
                      Enable
                    </>
                  )}
                </DropdownMenuItem>
                <DropdownMenuSeparator />
                <DropdownMenuItem
                  variant="destructive"
                  onClick={() => setDeleteOpen(true)}
                >
                  <Trash2 />
                  Delete
                </DropdownMenuItem>
              </DropdownMenuContent>
            </DropdownMenu>
          </div>
        </CardHeader>

        <CardContent className="space-y-2 pt-2">
          <div className="flex items-center gap-4 text-xs text-muted-foreground">
            <span className="truncate">{bot.category || "claude-sonnet"}</span>
            <span className="shrink-0">{lastActivity}</span>
          </div>
          <div className="flex items-center gap-1 text-xs text-muted-foreground">
            <MessageSquare className="size-3" />
            <span>{bot.version_count} version{bot.version_count !== 1 ? "s" : ""}</span>
          </div>
          {bot.description && (
            <p className="text-xs text-muted-foreground line-clamp-2">
              {bot.description}
            </p>
          )}
        </CardContent>

        <CardFooter className="pt-0">
          <Button variant="outline" size="sm" className="w-full" asChild>
            <Link to="/bots/$botId/chat" params={{ botId: bot.id }}>
              <MessageSquare className="size-3.5" />
              Chat
            </Link>
          </Button>
        </CardFooter>
      </Card>

      <AlertDialog open={deleteOpen} onOpenChange={setDeleteOpen}>
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>Delete {bot.name}?</AlertDialogTitle>
            <AlertDialogDescription>
              This action cannot be undone. This will permanently delete the bot
              and all its data including chat sessions and memories.
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>Cancel</AlertDialogCancel>
            <AlertDialogAction
              onClick={handleDelete}
              className="bg-destructive text-white hover:bg-destructive/90"
            >
              {deleteMutation.isPending ? "Deleting..." : "Delete"}
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </>
  );
}
