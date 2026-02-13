import { DiffEditor } from "@monaco-editor/react";
import { Loader2 } from "lucide-react";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogDescription,
} from "@/components/ui/dialog";
import { useThemeStore } from "@/stores/theme-store";

interface DiffViewerProps {
  open: boolean;
  onClose: () => void;
  /** The older version content (left side). */
  original: string;
  /** The newer version content (right side). */
  modified: string;
  /** Label for the original (older) pane. */
  originalLabel: string;
  /** Label for the modified (newer) pane. */
  modifiedLabel: string;
}

/**
 * Side-by-side diff viewer using Monaco DiffEditor.
 *
 * Opens as a large dialog overlay. Both sides are read-only.
 * Word-level diff highlighting is enabled by default by Monaco.
 */
export function DiffViewer({
  open,
  onClose,
  original,
  modified,
  originalLabel,
  modifiedLabel,
}: DiffViewerProps) {
  const theme = useThemeStore((s) => s.theme);
  const monacoTheme = theme === "light" ? "light" : "vs-dark";

  return (
    <Dialog open={open} onOpenChange={(isOpen) => !isOpen && onClose()}>
      <DialogContent className="sm:max-w-6xl h-[80vh] flex flex-col p-0 gap-0">
        <DialogHeader className="px-6 pt-6 pb-3">
          <DialogTitle>Compare Versions</DialogTitle>
          <DialogDescription>
            Side-by-side comparison of soul content between two versions.
          </DialogDescription>
        </DialogHeader>

        {/* Version labels */}
        <div className="flex border-y text-xs text-muted-foreground">
          <div className="flex-1 px-4 py-1.5 border-r font-medium">
            {originalLabel}
          </div>
          <div className="flex-1 px-4 py-1.5 font-medium">
            {modifiedLabel}
          </div>
        </div>

        {/* Diff editor */}
        <div className="flex-1 min-h-0">
          <DiffEditor
            original={original}
            modified={modified}
            language="markdown"
            theme={monacoTheme}
            loading={
              <div className="flex items-center justify-center h-full text-muted-foreground">
                <Loader2 className="size-5 animate-spin mr-2" />
                Loading diff...
              </div>
            }
            options={{
              readOnly: true,
              renderSideBySide: true,
              minimap: { enabled: false },
              lineNumbers: "on",
              wordWrap: "on",
              scrollBeyondLastLine: false,
              fontSize: 13,
              padding: { top: 8, bottom: 8 },
              automaticLayout: true,
              originalEditable: false,
            }}
          />
        </div>
      </DialogContent>
    </Dialog>
  );
}
