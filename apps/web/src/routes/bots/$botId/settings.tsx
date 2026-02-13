import { useState, useEffect } from "react";
import { createFileRoute, useNavigate } from "@tanstack/react-router";
import { useBot, useUpdateBot, useDeleteBot } from "@/hooks/use-bot-queries";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Textarea } from "@/components/ui/textarea";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
  AlertDialogTrigger,
} from "@/components/ui/alert-dialog";
import { Loader2, Save, Trash2 } from "lucide-react";

export const Route = createFileRoute("/bots/$botId/settings")({
  component: BotSettingsPage,
});

function BotSettingsPage() {
  const { botId } = Route.useParams();
  const navigate = useNavigate();
  const { data: bot } = useBot(botId);
  const updateMutation = useUpdateBot();
  const deleteMutation = useDeleteBot();

  const [name, setName] = useState("");
  const [description, setDescription] = useState("");
  const [isDirty, setIsDirty] = useState(false);

  // Sync form state when bot data loads
  useEffect(() => {
    if (bot) {
      setName(bot.name);
      setDescription(bot.description ?? "");
      setIsDirty(false);
    }
  }, [bot]);

  function handleNameChange(value: string) {
    setName(value);
    setIsDirty(true);
  }

  function handleDescriptionChange(value: string) {
    setDescription(value);
    setIsDirty(true);
  }

  function handleSave() {
    updateMutation.mutate(
      {
        id: botId,
        data: {
          name: name.trim() || undefined,
          description: description.trim() || undefined,
        },
      },
      { onSuccess: () => setIsDirty(false) },
    );
  }

  function handleDelete() {
    deleteMutation.mutate(botId, {
      onSuccess: () => navigate({ to: "/" }),
    });
  }

  if (!bot) return null;

  return (
    <div className="space-y-4 max-w-2xl">
      <Card>
        <CardHeader>
          <CardTitle>Bot Configuration</CardTitle>
          <CardDescription>
            Update your bot's name and description.
          </CardDescription>
        </CardHeader>
        <CardContent className="space-y-4">
          <div className="space-y-2">
            <Label htmlFor="bot-name">Name</Label>
            <Input
              id="bot-name"
              value={name}
              onChange={(e) => handleNameChange(e.target.value)}
              placeholder="Bot name"
            />
          </div>
          <div className="space-y-2">
            <Label htmlFor="bot-description">Description</Label>
            <Textarea
              id="bot-description"
              value={description}
              onChange={(e) => handleDescriptionChange(e.target.value)}
              placeholder="A short description of your bot"
              rows={3}
            />
          </div>
          <Button
            onClick={handleSave}
            disabled={!isDirty || !name.trim() || updateMutation.isPending}
          >
            {updateMutation.isPending ? (
              <Loader2 className="size-4 animate-spin" />
            ) : (
              <Save className="size-4" />
            )}
            Save Changes
          </Button>
        </CardContent>
      </Card>

      <Card className="border-destructive/50">
        <CardHeader>
          <CardTitle className="text-destructive">Danger Zone</CardTitle>
          <CardDescription>
            Permanently delete this bot and all its data.
          </CardDescription>
        </CardHeader>
        <CardContent>
          <AlertDialog>
            <AlertDialogTrigger asChild>
              <Button variant="destructive">
                <Trash2 className="size-4" />
                Delete Bot
              </Button>
            </AlertDialogTrigger>
            <AlertDialogContent>
              <AlertDialogHeader>
                <AlertDialogTitle>Delete {bot.name}?</AlertDialogTitle>
                <AlertDialogDescription>
                  This action cannot be undone. This will permanently delete the
                  bot and all its data including chat sessions and memories.
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
        </CardContent>
      </Card>
    </div>
  );
}
