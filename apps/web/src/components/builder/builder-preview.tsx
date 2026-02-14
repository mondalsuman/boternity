/**
 * BuilderPreview component -- live preview panel showing the bot config
 * as it is being assembled step by step.
 *
 * Displayed alongside the wizard steps (right panel on desktop,
 * below on mobile). Updates reactively as answers come in via the
 * BuilderPreview from the store.
 */

import { Bot, Brain, Cpu, Puzzle } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import type { BuilderPreview as BuilderPreviewType } from "@/lib/api/builder";

interface BuilderPreviewProps {
  preview: BuilderPreviewType | null;
}

export function BuilderPreview({ preview }: BuilderPreviewProps) {
  if (!preview) {
    return (
      <div className="flex flex-col items-center justify-center py-12 text-center">
        <Bot className="size-10 text-muted-foreground/30" />
        <p className="mt-3 text-sm text-muted-foreground">
          Your bot preview will appear here as you answer questions.
        </p>
      </div>
    );
  }

  return (
    <div className="space-y-5">
      {/* Bot identity */}
      <div className="space-y-2">
        <div className="flex items-center gap-2 text-xs font-medium uppercase tracking-wider text-muted-foreground">
          <Bot className="size-3.5" />
          Identity
        </div>
        {preview.name ? (
          <h3 className="text-lg font-semibold">{preview.name}</h3>
        ) : (
          <p className="text-sm italic text-muted-foreground">
            Name not set yet
          </p>
        )}
        {preview.description && (
          <p className="text-sm text-muted-foreground">
            {preview.description}
          </p>
        )}
      </div>

      {/* Personality */}
      {preview.personality_summary && (
        <div className="space-y-2">
          <div className="flex items-center gap-2 text-xs font-medium uppercase tracking-wider text-muted-foreground">
            <Brain className="size-3.5" />
            Personality
          </div>
          <p className="text-sm">{preview.personality_summary}</p>
        </div>
      )}

      {/* Model */}
      {preview.model && (
        <div className="space-y-2">
          <div className="flex items-center gap-2 text-xs font-medium uppercase tracking-wider text-muted-foreground">
            <Cpu className="size-3.5" />
            Model
          </div>
          <Badge variant="secondary" className="font-mono text-xs">
            {preview.model}
          </Badge>
        </div>
      )}

      {/* Skills */}
      {preview.skills.length > 0 && (
        <div className="space-y-2">
          <div className="flex items-center gap-2 text-xs font-medium uppercase tracking-wider text-muted-foreground">
            <Puzzle className="size-3.5" />
            Skills
          </div>
          <div className="flex flex-wrap gap-1.5">
            {preview.skills.map((skill) => (
              <Badge key={skill} variant="outline" className="text-xs">
                {skill}
              </Badge>
            ))}
          </div>
        </div>
      )}
    </div>
  );
}
