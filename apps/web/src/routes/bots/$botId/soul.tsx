import { createFileRoute } from "@tanstack/react-router";
import { SoulEditor } from "@/components/soul/soul-editor";

export const Route = createFileRoute("/bots/$botId/soul")({
  component: BotSoulPage,
});

function BotSoulPage() {
  const { botId } = Route.useParams();

  return (
    <div className="space-y-4">
      <div>
        <h2 className="text-lg font-semibold tracking-tight">Soul Editor</h2>
        <p className="text-sm text-muted-foreground">
          Edit the soul, identity, and user context files that shape this bot's personality.
        </p>
      </div>
      <SoulEditor botId={botId} />
    </div>
  );
}
