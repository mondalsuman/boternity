/**
 * BuilderReview component -- full review step shown when the builder
 * signals ReadyToAssemble.
 *
 * Shows:
 * - Structured summary of the bot configuration
 * - "Show raw files" toggle for SOUL.md / IDENTITY.md preview
 * - Skills list with remove option
 * - "Create Bot" primary action
 * - Success state with link to chat
 */

import { useState } from "react";
import {
  Bot,
  Brain,
  Check,
  Code,
  Cpu,
  ExternalLink,
  Loader2,
  Puzzle,
  Tag,
  X,
} from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card } from "@/components/ui/card";
import type { AssemblyResult, BuilderConfig } from "@/lib/api/builder";

interface BuilderReviewProps {
  config: BuilderConfig;
  onAssemble: (config: BuilderConfig) => Promise<AssemblyResult>;
  isLoading: boolean;
}

export function BuilderReview({
  config: initialConfig,
  onAssemble,
  isLoading,
}: BuilderReviewProps) {
  const [config, setConfig] = useState<BuilderConfig>(initialConfig);
  const [showRaw, setShowRaw] = useState(false);
  const [result, setResult] = useState<AssemblyResult | null>(null);
  const [error, setError] = useState<string | null>(null);

  function handleRemoveSkill(index: number) {
    setConfig((prev) => ({
      ...prev,
      skills: prev.skills.filter((_s, i) => i !== index),
    }));
  }

  async function handleCreate() {
    setError(null);
    try {
      const assemblyResult = await onAssemble(config);
      setResult(assemblyResult);
    } catch (err) {
      setError(
        err instanceof Error ? err.message : "Failed to create bot",
      );
    }
  }

  // Success state
  if (result) {
    return (
      <div className="flex flex-col items-center py-12 text-center">
        <div className="flex size-16 items-center justify-center rounded-full bg-green-500/10">
          <Check className="size-8 text-green-500" />
        </div>
        <h2 className="mt-4 text-xl font-semibold">Bot Created!</h2>
        <p className="mt-2 text-muted-foreground">
          <span className="font-medium text-foreground">
            {result.bot_name}
          </span>{" "}
          is ready to chat.
        </p>
        <div className="mt-6 flex gap-3">
          <Button asChild>
            <a href={`/chat?bot=${result.bot_id}`}>
              <ExternalLink className="size-4" />
              Start Chatting
            </a>
          </Button>
          <Button variant="outline" asChild>
            <a href={`/bots/${result.bot_id}`}>View Bot</a>
          </Button>
        </div>
      </div>
    );
  }

  return (
    <div className="space-y-6">
      <div>
        <h2 className="text-xl font-semibold">Review Your Bot</h2>
        <p className="text-sm text-muted-foreground">
          Review the configuration below. You can go back to change settings or
          create the bot.
        </p>
      </div>

      {/* Structured summary */}
      <div className="grid gap-4">
        {/* Identity */}
        <Card className="p-4">
          <div className="flex items-center gap-2 text-sm font-medium">
            <Bot className="size-4 text-muted-foreground" />
            Identity
          </div>
          <div className="mt-3 space-y-2">
            <div>
              <span className="text-xs text-muted-foreground">Name</span>
              <p className="font-medium">{config.name}</p>
            </div>
            <div>
              <span className="text-xs text-muted-foreground">
                Description
              </span>
              <p className="text-sm">{config.description}</p>
            </div>
            <div className="flex items-center gap-2">
              <span className="text-xs text-muted-foreground">Category</span>
              <Badge variant="secondary" className="text-xs">
                {config.category}
              </Badge>
            </div>
            {config.tags.length > 0 && (
              <div className="flex items-center gap-2">
                <Tag className="size-3 text-muted-foreground" />
                <div className="flex flex-wrap gap-1">
                  {config.tags.map((tag) => (
                    <Badge key={tag} variant="outline" className="text-xs">
                      {tag}
                    </Badge>
                  ))}
                </div>
              </div>
            )}
          </div>
        </Card>

        {/* Personality */}
        <Card className="p-4">
          <div className="flex items-center gap-2 text-sm font-medium">
            <Brain className="size-4 text-muted-foreground" />
            Personality
          </div>
          <div className="mt-3 space-y-2">
            <div>
              <span className="text-xs text-muted-foreground">Tone</span>
              <p className="text-sm">{config.personality.tone}</p>
            </div>
            <div>
              <span className="text-xs text-muted-foreground">Purpose</span>
              <p className="text-sm">{config.personality.purpose}</p>
            </div>
            {config.personality.traits.length > 0 && (
              <div>
                <span className="text-xs text-muted-foreground">Traits</span>
                <div className="mt-1 flex flex-wrap gap-1">
                  {config.personality.traits.map((trait) => (
                    <Badge key={trait} variant="outline" className="text-xs">
                      {trait}
                    </Badge>
                  ))}
                </div>
              </div>
            )}
            {config.personality.boundaries && (
              <div>
                <span className="text-xs text-muted-foreground">
                  Boundaries
                </span>
                <p className="text-sm">{config.personality.boundaries}</p>
              </div>
            )}
          </div>
        </Card>

        {/* Model */}
        <Card className="p-4">
          <div className="flex items-center gap-2 text-sm font-medium">
            <Cpu className="size-4 text-muted-foreground" />
            Model Configuration
          </div>
          <div className="mt-3 space-y-2">
            <div>
              <span className="text-xs text-muted-foreground">Model</span>
              <p className="font-mono text-sm">{config.model_config.model}</p>
            </div>
            <div className="flex gap-6">
              <div>
                <span className="text-xs text-muted-foreground">
                  Temperature
                </span>
                <p className="text-sm">{config.model_config.temperature}</p>
              </div>
              <div>
                <span className="text-xs text-muted-foreground">
                  Max Tokens
                </span>
                <p className="text-sm">
                  {config.model_config.max_tokens.toLocaleString()}
                </p>
              </div>
            </div>
          </div>
        </Card>

        {/* Skills */}
        <Card className="p-4">
          <div className="flex items-center gap-2 text-sm font-medium">
            <Puzzle className="size-4 text-muted-foreground" />
            Skills
            <Badge variant="secondary" className="ml-auto text-xs">
              {config.skills.length}
            </Badge>
          </div>
          {config.skills.length > 0 ? (
            <div className="mt-3 space-y-2">
              {config.skills.map((skill, i) => (
                <div
                  key={`${skill.name}-${i}`}
                  className="flex items-center justify-between rounded-md border p-2"
                >
                  <div>
                    <p className="text-sm font-medium">{skill.name}</p>
                    <p className="text-xs text-muted-foreground">
                      {skill.description}
                    </p>
                  </div>
                  <button
                    type="button"
                    onClick={() => handleRemoveSkill(i)}
                    className="rounded-md p-1 text-muted-foreground transition-colors hover:bg-destructive/10 hover:text-destructive"
                    title="Remove skill"
                  >
                    <X className="size-4" />
                  </button>
                </div>
              ))}
            </div>
          ) : (
            <p className="mt-3 text-sm text-muted-foreground">
              No skills attached. You can add skills later from the bot settings.
            </p>
          )}
        </Card>
      </div>

      {/* Raw files toggle */}
      <div>
        <button
          type="button"
          onClick={() => setShowRaw(!showRaw)}
          className="flex items-center gap-2 text-sm text-muted-foreground transition-colors hover:text-foreground"
        >
          <Code className="size-4" />
          {showRaw ? "Hide" : "Show"} raw files
        </button>
        {showRaw && (
          <div className="mt-3 space-y-3">
            <RawFilePreview
              filename="IDENTITY.md"
              content={generateIdentityMd(config)}
            />
            <RawFilePreview
              filename="SOUL.md"
              content={generateSoulMd(config)}
            />
          </div>
        )}
      </div>

      {/* Error */}
      {error && <p className="text-sm text-destructive">{error}</p>}

      {/* Create button */}
      <div className="flex justify-end">
        <Button onClick={handleCreate} disabled={isLoading} size="lg">
          {isLoading && <Loader2 className="size-4 animate-spin" />}
          Create Bot
        </Button>
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Raw file preview
// ---------------------------------------------------------------------------

function RawFilePreview({
  filename,
  content,
}: {
  filename: string;
  content: string;
}) {
  return (
    <div className="rounded-md border">
      <div className="flex items-center border-b px-3 py-1.5">
        <span className="font-mono text-xs text-muted-foreground">
          {filename}
        </span>
      </div>
      <pre className="overflow-auto p-3 text-xs leading-relaxed">{content}</pre>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Raw file generators (client-side preview only)
// ---------------------------------------------------------------------------

function generateIdentityMd(config: BuilderConfig): string {
  const lines = [
    "---",
    `model: ${config.model_config.model}`,
    `temperature: ${config.model_config.temperature}`,
    `max_tokens: ${config.model_config.max_tokens}`,
    "---",
    "",
    `# ${config.name}`,
    "",
    config.description,
  ];
  return lines.join("\n");
}

function generateSoulMd(config: BuilderConfig): string {
  const lines = [
    `# ${config.name}`,
    "",
    `## Purpose`,
    "",
    config.personality.purpose,
    "",
    `## Personality`,
    "",
    `**Tone:** ${config.personality.tone}`,
  ];

  if (config.personality.traits.length > 0) {
    lines.push("");
    lines.push("**Traits:**");
    for (const trait of config.personality.traits) {
      lines.push(`- ${trait}`);
    }
  }

  if (config.personality.boundaries) {
    lines.push("");
    lines.push("## Boundaries");
    lines.push("");
    lines.push(config.personality.boundaries);
  }

  return lines.join("\n");
}
