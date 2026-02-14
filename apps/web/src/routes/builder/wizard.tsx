/**
 * Wizard page -- /builder/wizard
 *
 * Step-by-step wizard layout with:
 * - Phase indicator bar at top
 * - Current step content (WizardStep or BuilderReview)
 * - Live preview panel (right on desktop, below on mobile)
 * - Back/Next navigation footer
 *
 * The wizard does not control steps directly -- it renders whatever
 * BuilderTurn the API returns. The step indicator updates based on the
 * phase field in the turn / state summary.
 */

import { useEffect, useRef } from "react";
import { createFileRoute, useNavigate } from "@tanstack/react-router";
import { ArrowLeft, Loader2 } from "lucide-react";
import { cn } from "@/lib/utils";
import { PHASE_LABELS, PHASE_ORDER } from "@/lib/api/builder";
import type { BuilderAnswer, BuilderPhase } from "@/lib/api/builder";
import { useBuilderStore } from "@/stores/builder-store";
import { Button } from "@/components/ui/button";
import { Card } from "@/components/ui/card";
import { WizardStep } from "@/components/builder/wizard-step";
import { BuilderPreview } from "@/components/builder/builder-preview";
import { BuilderReview } from "@/components/builder/builder-review";

export const Route = createFileRoute("/builder/wizard")({
  component: WizardPage,
  validateSearch: (search: Record<string, unknown>) => ({
    description: (search.description as string) || "",
  }),
});

function WizardPage() {
  const navigate = useNavigate();
  const { description } = Route.useSearch();
  const {
    sessionId,
    currentTurn,
    phase,
    history,
    preview,
    isLoading,
    error,
    startSession,
    submitAnswer,
    goBack,
    assemble,
    reset,
  } = useBuilderStore();

  // If we landed on wizard without a session, start one (once)
  const startedRef = useRef(false);
  useEffect(() => {
    if (!sessionId && description && !isLoading && !startedRef.current) {
      startedRef.current = true;
      startSession(description);
    }
  }, [sessionId, description, isLoading, startSession]);

  // Calculate step index for the progress bar
  const currentStepIndex = PHASE_ORDER.indexOf(phase);

  function handleAnswer(answer: BuilderAnswer) {
    submitAnswer(answer);
  }

  function handleBack() {
    if (history.length === 0) {
      // Go back to builder landing
      reset();
      navigate({ to: "/builder" });
      return;
    }
    goBack();
  }

  // Loading state before first turn arrives
  if (!currentTurn) {
    return (
      <div className="flex items-center justify-center py-24">
        <div className="flex flex-col items-center gap-3">
          <Loader2 className="size-8 animate-spin text-muted-foreground" />
          <p className="text-sm text-muted-foreground">
            Setting up your builder session...
          </p>
        </div>
      </div>
    );
  }

  const isReviewStep = currentTurn.action === "ready_to_assemble";

  return (
    <div className="mx-auto max-w-5xl p-4 md:p-6">
      {/* Step indicator */}
      <StepIndicator currentPhase={phase} />

      {/* Main content area */}
      <div className="mt-8 grid gap-6 lg:grid-cols-[1fr,300px]">
        {/* Left: step content */}
        <div className="min-w-0">
          {isReviewStep && currentTurn.action === "ready_to_assemble" ? (
            <BuilderReview
              config={currentTurn.config}
              onAssemble={assemble}
              isLoading={isLoading}
            />
          ) : (
            <>
              <WizardStep
                turn={currentTurn}
                onAnswer={handleAnswer}
                isLoading={isLoading}
              />

              {/* Error display */}
              {error && (
                <p className="mt-4 text-sm text-destructive">{error}</p>
              )}

              {/* Navigation footer */}
              <div className="mt-8 flex items-center justify-between border-t pt-4">
                <Button
                  variant="ghost"
                  onClick={handleBack}
                  disabled={isLoading}
                >
                  <ArrowLeft className="size-4" />
                  Back
                </Button>

                {/* Loading indicator (answers auto-submit on click) */}
                {isLoading && (
                  <div className="flex items-center gap-2 text-sm text-muted-foreground">
                    <Loader2 className="size-4 animate-spin" />
                    Processing...
                  </div>
                )}
              </div>
            </>
          )}
        </div>

        {/* Right: live preview */}
        <div className="hidden lg:block">
          <Card className="sticky top-20 p-4">
            <h3 className="mb-3 text-xs font-medium uppercase tracking-wider text-muted-foreground">
              Preview
            </h3>
            <BuilderPreview preview={preview} />
          </Card>
        </div>
      </div>

      {/* Mobile preview (below content) */}
      <div className="mt-6 lg:hidden">
        <details className="group">
          <summary className="cursor-pointer text-sm font-medium text-muted-foreground hover:text-foreground">
            Show Preview
          </summary>
          <Card className="mt-2 p-4">
            <BuilderPreview preview={preview} />
          </Card>
        </details>
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Step indicator
// ---------------------------------------------------------------------------

interface StepIndicatorProps {
  currentPhase: BuilderPhase;
}

function StepIndicator({ currentPhase }: StepIndicatorProps) {
  const currentIndex = PHASE_ORDER.indexOf(currentPhase);

  return (
    <div className="space-y-2">
      {/* Progress bars */}
      <div className="flex gap-1.5">
        {PHASE_ORDER.map((phase, i) => (
          <div
            key={phase}
            className={cn(
              "h-1 flex-1 rounded-full transition-colors",
              i <= currentIndex ? "bg-primary" : "bg-muted",
            )}
          />
        ))}
      </div>

      {/* Phase labels */}
      <div className="flex justify-between">
        {PHASE_ORDER.map((phase, i) => (
          <span
            key={phase}
            className={cn(
              "text-xs transition-colors",
              i <= currentIndex
                ? "font-medium text-foreground"
                : "text-muted-foreground",
              i === currentIndex && "text-primary",
            )}
          >
            {PHASE_LABELS[phase]}
          </span>
        ))}
      </div>
    </div>
  );
}
