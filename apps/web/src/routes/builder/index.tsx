/**
 * Builder landing page -- /builder
 *
 * Entry point for bot creation via the step-by-step wizard.
 * Shows a description input to start a new session, a link to Forge chat
 * (Plan 10), and a "Resume Draft" section for existing drafts.
 */

import { useState } from "react";
import { createFileRoute, useNavigate } from "@tanstack/react-router";
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import {
  ArrowRight,
  FileText,
  Loader2,
  MessageCircle,
  Trash2,
  Wand2,
} from "lucide-react";
import { Button } from "@/components/ui/button";
import { Card } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { listDrafts, deleteSession } from "@/lib/api/builder";
import type { DraftSummary } from "@/lib/api/builder";
import { useBuilderStore } from "@/stores/builder-store";

export const Route = createFileRoute("/builder/")({
  component: BuilderLandingPage,
});

function BuilderLandingPage() {
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const [description, setDescription] = useState("");
  const { startSession, isLoading: storeLoading, error } = useBuilderStore();

  // Fetch existing drafts
  const { data: drafts } = useQuery({
    queryKey: ["builder-drafts"],
    queryFn: listDrafts,
    retry: 1,
  });

  // Delete draft mutation
  const deleteDraft = useMutation({
    mutationFn: (sessionId: string) => deleteSession(sessionId),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["builder-drafts"] });
    },
  });

  async function handleStartWizard(e: React.FormEvent) {
    e.preventDefault();
    const trimmed = description.trim();
    if (!trimmed || storeLoading) return;

    await startSession(trimmed);
    navigate({ to: "/builder/wizard", search: { description: trimmed } });
  }

  async function handleResumeDraft(draft: DraftSummary) {
    // Start session with the draft description and navigate
    await startSession(draft.initial_description);
    navigate({
      to: "/builder/wizard",
      search: { description: draft.initial_description },
    });
  }

  return (
    <div className="mx-auto max-w-2xl p-4 md:p-6">
      {/* Header */}
      <div className="space-y-2 text-center">
        <div className="mx-auto flex size-12 items-center justify-center rounded-full bg-primary/10">
          <Wand2 className="size-6 text-primary" />
        </div>
        <h1 className="text-2xl font-bold tracking-tight">
          Create a New Bot
        </h1>
        <p className="text-muted-foreground">
          Describe the bot you want to create and we will guide you through the
          setup step by step.
        </p>
      </div>

      {/* Description input */}
      <form onSubmit={handleStartWizard} className="mt-8 space-y-4">
        <div className="space-y-2">
          <label htmlFor="bot-description" className="text-sm font-medium">
            What kind of bot do you want to build?
          </label>
          <Input
            id="bot-description"
            placeholder="e.g., A coding assistant that helps with Rust and TypeScript..."
            value={description}
            onChange={(e) => setDescription(e.target.value)}
            className="h-12"
            autoFocus
          />
        </div>
        <Button
          type="submit"
          disabled={!description.trim() || storeLoading}
          className="w-full"
          size="lg"
        >
          {storeLoading ? (
            <Loader2 className="size-4 animate-spin" />
          ) : (
            <ArrowRight className="size-4" />
          )}
          Start Wizard
        </Button>
      </form>

      {/* Error display */}
      {error && (
        <p className="mt-4 text-center text-sm text-destructive">{error}</p>
      )}

      {/* Forge chat links */}
      <div className="mt-6 flex flex-col items-center gap-2">
        <p className="text-sm text-muted-foreground">
          Prefer a conversational approach?{" "}
          <a
            href="/builder/forge"
            className="inline-flex items-center gap-1 font-medium text-primary hover:underline"
          >
            <MessageCircle className="size-3.5" />
            Chat with Forge
          </a>
        </p>
        <p className="text-sm text-muted-foreground">
          Or{" "}
          <a
            href="/builder/forge?mode=skill"
            className="inline-flex items-center gap-1 font-medium text-primary hover:underline"
          >
            <MessageCircle className="size-3.5" />
            Create a Skill with Forge
          </a>
        </p>
      </div>

      {/* Resume drafts */}
      {drafts && drafts.length > 0 && (
        <div className="mt-10 space-y-3">
          <h2 className="text-sm font-medium text-muted-foreground">
            Resume a Draft
          </h2>
          <div className="space-y-2">
            {drafts.map((draft) => (
              <Card
                key={draft.session_id}
                className="flex items-center justify-between p-3"
              >
                <button
                  type="button"
                  onClick={() => handleResumeDraft(draft)}
                  className="flex flex-1 items-start gap-3 text-left"
                >
                  <FileText className="mt-0.5 size-4 text-muted-foreground" />
                  <div className="min-w-0 flex-1">
                    <p className="truncate text-sm font-medium">
                      {draft.initial_description}
                    </p>
                    <p className="text-xs text-muted-foreground">
                      Phase: {draft.phase} -- Updated{" "}
                      {new Date(draft.updated_at).toLocaleDateString()}
                    </p>
                  </div>
                </button>
                <button
                  type="button"
                  onClick={(e) => {
                    e.stopPropagation();
                    deleteDraft.mutate(draft.session_id);
                  }}
                  className="rounded-md p-1.5 text-muted-foreground transition-colors hover:bg-destructive/10 hover:text-destructive"
                  title="Delete draft"
                >
                  <Trash2 className="size-4" />
                </button>
              </Card>
            ))}
          </div>
        </div>
      )}
    </div>
  );
}
