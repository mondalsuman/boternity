import { format } from "date-fns";
import { Loader2, RotateCcw } from "lucide-react";
import {
  AlertDialog,
  AlertDialogContent,
  AlertDialogHeader,
  AlertDialogTitle,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogCancel,
} from "@/components/ui/alert-dialog";
import { Button } from "@/components/ui/button";
import { useRollbackSoul } from "@/hooks/use-soul-queries";
import type { SoulVersion } from "@/types/soul";

interface RollbackDialogProps {
  botId: string;
  version: SoulVersion | null;
  open: boolean;
  onClose: () => void;
}

/**
 * Rollback confirmation dialog with version content preview.
 *
 * Creates a NEW version with the old content (preserves linear history).
 * After rollback, the soul editor will reload the new current content.
 */
export function RollbackDialog({
  botId,
  version,
  open,
  onClose,
}: RollbackDialogProps) {
  const rollback = useRollbackSoul(botId);

  const handleRestore = () => {
    if (!version) return;
    rollback.mutate(version.version, {
      onSuccess: () => {
        onClose();
      },
    });
  };

  if (!version) return null;

  const dateLabel = format(
    new Date(version.created_at),
    "MMM d, yyyy 'at' h:mm a",
  );

  return (
    <AlertDialog open={open} onOpenChange={(isOpen) => !isOpen && onClose()}>
      <AlertDialogContent className="sm:max-w-xl">
        <AlertDialogHeader>
          <AlertDialogTitle className="flex items-center gap-2">
            <RotateCcw className="size-4" />
            Restore Version {version.version}?
          </AlertDialogTitle>
          <AlertDialogDescription>
            This will create a new version with the content from version{" "}
            {version.version} ({dateLabel}). Your current content will be
            preserved as the previous version.
          </AlertDialogDescription>
        </AlertDialogHeader>

        {/* Content preview */}
        <div className="rounded-md border bg-muted/30 max-h-[200px] overflow-auto">
          <div className="sticky top-0 bg-muted/50 border-b px-3 py-1.5">
            <span className="text-[10px] font-medium text-muted-foreground uppercase tracking-wider">
              Version {version.version} content preview
            </span>
          </div>
          <pre className="px-3 py-2 text-xs text-muted-foreground whitespace-pre-wrap font-mono leading-relaxed">
            {version.content}
          </pre>
        </div>

        <AlertDialogFooter>
          <AlertDialogCancel disabled={rollback.isPending}>
            Cancel
          </AlertDialogCancel>
          <Button
            variant="destructive"
            onClick={handleRestore}
            disabled={rollback.isPending}
          >
            {rollback.isPending ? (
              <>
                <Loader2 className="size-4 animate-spin" />
                Restoring...
              </>
            ) : (
              <>
                <RotateCcw className="size-4" />
                Restore
              </>
            )}
          </Button>
        </AlertDialogFooter>
      </AlertDialogContent>
    </AlertDialog>
  );
}
