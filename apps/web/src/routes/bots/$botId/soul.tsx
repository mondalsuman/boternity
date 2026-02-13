import { useState } from "react";
import { createFileRoute } from "@tanstack/react-router";
import { History } from "lucide-react";
import { Button } from "@/components/ui/button";
import {
  Sheet,
  SheetContent,
  SheetHeader,
  SheetTitle,
  SheetDescription,
} from "@/components/ui/sheet";
import { useIsMobile } from "@/hooks/use-mobile";
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
  const isMobile = useIsMobile();

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

  const timelineProps = {
    botId,
    currentVersion,
    open: timelineOpen,
    onToggle: () => setTimelineOpen(false),
    onCompare: (original: SoulVersion, modified: SoulVersion) =>
      setDiffVersions({ original, modified }),
    onRestore: (version: SoulVersion) => setRollbackVersion(version),
  };

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
          <span className="hidden sm:inline">History</span>
        </Button>
      </div>

      <div className="flex min-h-[300px] md:min-h-[500px]">
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

        {/* Desktop: version timeline as right panel */}
        {!isMobile && <VersionTimeline {...timelineProps} />}
      </div>

      {/* Mobile: version timeline as bottom sheet */}
      {isMobile && (
        <Sheet open={timelineOpen} onOpenChange={setTimelineOpen}>
          <SheetContent side="bottom" className="h-[70vh] p-0">
            <SheetHeader className="sr-only">
              <SheetTitle>Version History</SheetTitle>
              <SheetDescription>Soul version history</SheetDescription>
            </SheetHeader>
            <VersionTimeline {...timelineProps} open={true} />
          </SheetContent>
        </Sheet>
      )}
    </div>
  );
}
