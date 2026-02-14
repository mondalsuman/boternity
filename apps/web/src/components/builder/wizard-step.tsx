/**
 * WizardStep component -- renders a single BuilderTurn as a wizard step.
 *
 * Handles all turn types:
 * - AskQuestion: question text, option cards (radio-style), optional free text
 * - ShowPreview: inline preview display
 * - ReadyToAssemble: delegates to review component
 * - Clarify: message with text input
 */

import { useState } from "react";
import { MessageSquare } from "lucide-react";
import { cn } from "@/lib/utils";
import type { BuilderTurn, BuilderAnswer } from "@/lib/api/builder";
import { Card } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";

interface WizardStepProps {
  turn: BuilderTurn;
  onAnswer: (answer: BuilderAnswer) => void;
  isLoading: boolean;
}

export function WizardStep({ turn, onAnswer, isLoading }: WizardStepProps) {
  switch (turn.action) {
    case "ask_question":
      return (
        <AskQuestionStep
          question={turn.question}
          options={turn.options}
          allowFreeText={turn.allow_free_text}
          phaseLabel={turn.phase_label}
          onAnswer={onAnswer}
          isLoading={isLoading}
        />
      );
    case "show_preview":
      return (
        <div className="space-y-4">
          <p className="text-muted-foreground">
            Here is a preview of your bot so far. Click Next to continue.
          </p>
        </div>
      );
    case "clarify":
      return (
        <ClarifyStep
          message={turn.message}
          onAnswer={onAnswer}
          isLoading={isLoading}
        />
      );
    case "ready_to_assemble":
      // Handled by the parent wizard page (review step)
      return null;
  }
}

// ---------------------------------------------------------------------------
// AskQuestion step
// ---------------------------------------------------------------------------

interface AskQuestionStepProps {
  question: string;
  options: { id: string; label: string; description?: string }[];
  allowFreeText: boolean;
  phaseLabel?: string;
  onAnswer: (answer: BuilderAnswer) => void;
  isLoading: boolean;
}

function AskQuestionStep({
  question,
  options,
  allowFreeText,
  phaseLabel,
  onAnswer,
  isLoading,
}: AskQuestionStepProps) {
  const [selectedIndex, setSelectedIndex] = useState<number | null>(null);
  const [freeText, setFreeText] = useState("");
  const [useFreeText, setUseFreeText] = useState(false);

  function handleOptionClick(index: number) {
    if (isLoading) return;
    setUseFreeText(false);
    setSelectedIndex(index);
    // Auto-submit on option click for smooth flow
    onAnswer({ OptionIndex: index });
  }

  function handleFreeTextSubmit() {
    if (isLoading || !freeText.trim()) return;
    onAnswer({ FreeText: freeText.trim() });
  }

  return (
    <div className="space-y-6">
      {phaseLabel && (
        <p className="text-xs font-medium uppercase tracking-wider text-muted-foreground">
          {phaseLabel}
        </p>
      )}

      <h2 className="text-xl font-semibold">{question}</h2>

      {/* Option cards */}
      <div className="grid gap-3">
        {options.map((option, index) => (
          <Card
            key={option.id}
            role="button"
            tabIndex={0}
            onClick={() => handleOptionClick(index)}
            onKeyDown={(e) => {
              if (e.key === "Enter" || e.key === " ") {
                e.preventDefault();
                handleOptionClick(index);
              }
            }}
            className={cn(
              "cursor-pointer p-4 transition-all hover:border-primary/50",
              selectedIndex === index && !useFreeText
                ? "border-primary ring-2 ring-primary/20 bg-accent/50"
                : "border-border",
              isLoading && "pointer-events-none opacity-60",
            )}
          >
            <div className="flex items-start gap-3">
              <div
                className={cn(
                  "mt-0.5 flex size-5 shrink-0 items-center justify-center rounded-full border-2 transition-colors",
                  selectedIndex === index && !useFreeText
                    ? "border-primary bg-primary"
                    : "border-muted-foreground/30",
                )}
              >
                {selectedIndex === index && !useFreeText && (
                  <div className="size-2 rounded-full bg-primary-foreground" />
                )}
              </div>
              <div className="space-y-1">
                <p className="font-medium leading-tight">{option.label}</p>
                {option.description && (
                  <p className="text-sm text-muted-foreground">
                    {option.description}
                  </p>
                )}
              </div>
            </div>
          </Card>
        ))}
      </div>

      {/* Free text option */}
      {allowFreeText && (
        <div className="space-y-3">
          <button
            type="button"
            onClick={() => {
              setUseFreeText(true);
              setSelectedIndex(null);
            }}
            className={cn(
              "flex items-center gap-2 text-sm transition-colors",
              useFreeText
                ? "text-primary font-medium"
                : "text-muted-foreground hover:text-foreground",
            )}
          >
            <MessageSquare className="size-4" />
            Or type your own answer
          </button>

          {useFreeText && (
            <div className="flex gap-2">
              <div className="flex-1 space-y-1.5">
                <Label htmlFor="free-text" className="sr-only">
                  Your answer
                </Label>
                <Input
                  id="free-text"
                  placeholder="Type your answer..."
                  value={freeText}
                  onChange={(e) => setFreeText(e.target.value)}
                  onKeyDown={(e) => {
                    if (e.key === "Enter") {
                      e.preventDefault();
                      handleFreeTextSubmit();
                    }
                  }}
                  disabled={isLoading}
                  autoFocus
                />
              </div>
              <button
                type="button"
                onClick={handleFreeTextSubmit}
                disabled={isLoading || !freeText.trim()}
                className="rounded-md bg-primary px-4 py-2 text-sm font-medium text-primary-foreground transition-colors hover:bg-primary/90 disabled:opacity-50"
              >
                Submit
              </button>
            </div>
          )}
        </div>
      )}
    </div>
  );
}

// ---------------------------------------------------------------------------
// Clarify step
// ---------------------------------------------------------------------------

interface ClarifyStepProps {
  message: string;
  onAnswer: (answer: BuilderAnswer) => void;
  isLoading: boolean;
}

function ClarifyStep({ message, onAnswer, isLoading }: ClarifyStepProps) {
  const [text, setText] = useState("");

  function handleSubmit() {
    if (isLoading || !text.trim()) return;
    onAnswer({ FreeText: text.trim() });
  }

  return (
    <div className="space-y-6">
      <div className="rounded-lg border border-amber-500/20 bg-amber-500/5 p-4">
        <p className="text-sm font-medium text-amber-600 dark:text-amber-400">
          Clarification needed
        </p>
        <p className="mt-1 text-sm text-muted-foreground">{message}</p>
      </div>

      <div className="space-y-2">
        <Label htmlFor="clarify-input">Your response</Label>
        <div className="flex gap-2">
          <Input
            id="clarify-input"
            placeholder="Provide more details..."
            value={text}
            onChange={(e) => setText(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter") {
                e.preventDefault();
                handleSubmit();
              }
            }}
            disabled={isLoading}
            autoFocus
          />
          <button
            type="button"
            onClick={handleSubmit}
            disabled={isLoading || !text.trim()}
            className="rounded-md bg-primary px-4 py-2 text-sm font-medium text-primary-foreground transition-colors hover:bg-primary/90 disabled:opacity-50"
          >
            Submit
          </button>
        </div>
      </div>
    </div>
  );
}
