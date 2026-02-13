import { useState } from "react";
import { createFileRoute } from "@tanstack/react-router";
import { History } from "lucide-react";
import { Button } from "@/components/ui/button";
import { SoulEditor } from "@/components/soul/soul-editor";
import { VersionTimeline } from "@/components/soul/version-timeline";
import { useSoul } from "@/hooks/use-soul-queries";
import type { SoulVersion } from "@/types/soul";

export const Route = createFileRoute("/bots/$botId/soul")({
  component: BotSoulPage,
});

function BotSoulPage() {
  const { botId } = Route.useParams();
  const { data: soul } = useSoul(botId);

  // Version timeline panel state
  const [timelineOpen, setTimelineOpen] = useState(false);

  // Diff viewer state
  const [diffVersions, setDiffVersions] = useState<{
    original: SoulVersion;
    modified: SoulVersion;
  } | null>(null);

  // Rollback dialog state
  const [rollbackVersion, setRollbackVersion] = useState<SoulVersion | null>(
    null,
  );

  const currentVersion = soul?.version ?? 1;

  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between">
        <div>
          <h2 className="text-lg font-semibold tracking-tight">Soul Editor</h2>
          <p className="text-sm text-muted-foreground">
            Edit the soul, identity, and user context files that shape this
            bot's personality.
          </p>
        </div>
        <Button
          variant="outline"
          size="sm"
          onClick={() => setTimelineOpen((prev) => !prev)}
          className="gap-1.5"
        >
          <History className="size-4" />
          History
        </Button>
      </div>

      <div className="flex min-h-[500px]">
        {/* Editor + Preview (flex-1) */}
        <div className="flex-1 min-w-0">
          <SoulEditor
            botId={botId}
            diffVersions={diffVersions}
            onCloseDiff={() => setDiffVersions(null)}
            rollbackVersion={rollbackVersion}
            onCloseRollback={() => setRollbackVersion(null)}
          />
        </div>

        {/* Version timeline (right, collapsible) */}
        <VersionTimeline
          botId={botId}
          currentVersion={currentVersion}
          open={timelineOpen}
          onToggle={() => setTimelineOpen(false)}
          onCompare={(original, modified) => setDiffVersions({ original, modified })}
          onRestore={(version) => setRollbackVersion(version)}
        />
      </div>
    </div>
  );
}
