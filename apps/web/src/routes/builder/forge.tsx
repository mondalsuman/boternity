/**
 * Forge chat route -- /builder/forge
 *
 * Conversational bot and skill creation via the Forge character.
 * Users describe what they want and Forge guides them through
 * a multi-turn conversation with interactive option buttons.
 *
 * Layout: left side is the chat area, right side is a live preview panel
 * (hidden on mobile, visible on lg+ breakpoint).
 *
 * Supports two modes:
 * - Bot creation (default): ?mode=bot&description=...
 * - Skill creation: ?mode=skill&description=...
 *
 * Intent detection from free text: keywords like "skill" route to
 * skill creation; everything else defaults to bot creation.
 */

import { useState, useCallback, useEffect, useRef } from "react";
import { createFileRoute, useNavigate } from "@tanstack/react-router";
import { Hammer, Loader2, Send, Wifi, WifiOff } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { BuilderPreview } from "@/components/builder/builder-preview";
import { ForgeMessage } from "@/components/builder/forge-message";
import { useBuilderWs } from "@/hooks/use-builder-ws";
import { useForgeStore } from "@/stores/forge-store";
import type { BuilderAnswer, BuilderConfig } from "@/lib/api/builder";
import type { SkillRequestDto } from "@/hooks/use-builder-ws";

// ---------------------------------------------------------------------------
// Route definition
// ---------------------------------------------------------------------------

export const Route = createFileRoute("/builder/forge")({
  component: ForgePage,
  validateSearch: (search: Record<string, unknown>) => ({
    mode: (search.mode as string) || undefined,
    description: (search.description as string) || undefined,
  }),
});

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const FORGE_GREETING =
  "Hey there! I'm **Forge**, your bot and skill builder. I can help you create a new bot with a unique personality, or build a standalone skill.\n\nWhat would you like to create? Just describe it and I'll guide you through the process.";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/** Simple keyword detection for initial intent routing. */
function detectMode(text: string): "bot" | "skill" {
  const lower = text.toLowerCase();
  if (
    lower.includes("skill") &&
    !lower.includes("bot") &&
    !lower.includes("assistant") &&
    !lower.includes("agent")
  ) {
    return "skill";
  }
  return "bot";
}

/** Generate a session ID (UUID v4). */
function generateSessionId(): string {
  return crypto.randomUUID();
}

// ---------------------------------------------------------------------------
// Typing indicator
// ---------------------------------------------------------------------------

function TypingIndicator() {
  return (
    <div className="flex gap-3">
      <div className="flex-shrink-0 w-8 h-8 rounded-full bg-amber-500/20 flex items-center justify-center">
        <Hammer className="w-4 h-4 text-amber-500" />
      </div>
      <div className="bg-muted rounded-2xl rounded-bl-md px-4 py-3">
        <div className="flex gap-1.5">
          <span className="w-2 h-2 rounded-full bg-muted-foreground/40 animate-bounce [animation-delay:0ms]" />
          <span className="w-2 h-2 rounded-full bg-muted-foreground/40 animate-bounce [animation-delay:150ms]" />
          <span className="w-2 h-2 rounded-full bg-muted-foreground/40 animate-bounce [animation-delay:300ms]" />
        </div>
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Page component
// ---------------------------------------------------------------------------

function ForgePage() {
  const navigate = useNavigate();
  const { mode: searchMode, description: searchDescription } =
    Route.useSearch();

  const [inputValue, setInputValue] = useState("");
  const [hasStarted, setHasStarted] = useState(false);
  const messagesEndRef = useRef<HTMLDivElement>(null);
  const initRef = useRef(false);

  // Store state
  const messages = useForgeStore((s) => s.messages);
  const mode = useForgeStore((s) => s.mode);
  const preview = useForgeStore((s) => s.preview);
  const isAssembled = useForgeStore((s) => s.isAssembled);
  const assemblyResult = useForgeStore((s) => s.assemblyResult);
  const skillResult = useForgeStore((s) => s.skillResult);
  const isWaiting = useForgeStore((s) => s.isWaiting);
  const sessionId = useForgeStore((s) => s.sessionId);

  const setSessionId = useForgeStore((s) => s.setSessionId);
  const setMode = useForgeStore((s) => s.setMode);
  const addForgeMessage = useForgeStore((s) => s.addForgeMessage);
  const addUserMessage = useForgeStore((s) => s.addUserMessage);
  const reset = useForgeStore((s) => s.reset);

  // WebSocket connection
  const {
    isConnected,
    sendStartBot,
    sendStartSkill,
    sendAnswer,
    sendAssembleBot,
    sendCreateSkill,
  } = useBuilderWs(sessionId);

  // -----------------------------------------------------------------------
  // Initialize session on mount
  // -----------------------------------------------------------------------

  useEffect(() => {
    if (initRef.current) return;
    initRef.current = true;

    // Reset store for fresh session
    reset();
    const sid = generateSessionId();
    setSessionId(sid);

    // Add greeting message as a clarify turn
    addForgeMessage({
      action: "clarify" as const,
      message: FORGE_GREETING,
    });
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // -----------------------------------------------------------------------
  // Auto-start from query params
  // -----------------------------------------------------------------------

  useEffect(() => {
    if (hasStarted || !isConnected || !sessionId) return;

    if (searchDescription) {
      const resolvedMode = searchMode === "skill" ? "skill" : "bot";
      setMode(resolvedMode);
      addUserMessage(searchDescription);
      setHasStarted(true);

      if (resolvedMode === "skill") {
        sendStartSkill(searchDescription);
      } else {
        sendStartBot(searchDescription);
      }
    }
  }, [
    isConnected,
    sessionId,
    hasStarted,
    searchMode,
    searchDescription,
    setMode,
    addUserMessage,
    sendStartBot,
    sendStartSkill,
  ]);

  // -----------------------------------------------------------------------
  // Auto-scroll to bottom
  // -----------------------------------------------------------------------

  useEffect(() => {
    messagesEndRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [messages, isWaiting]);

  // -----------------------------------------------------------------------
  // Handlers
  // -----------------------------------------------------------------------

  const handleSend = useCallback(() => {
    const text = inputValue.trim();
    if (!text || isWaiting) return;

    setInputValue("");
    addUserMessage(text);

    // If no session started yet, detect mode and start
    if (!hasStarted) {
      const detectedMode = detectMode(text);
      setMode(detectedMode);
      setHasStarted(true);

      if (detectedMode === "skill") {
        sendStartSkill(text);
      } else {
        sendStartBot(text);
      }
      return;
    }

    // Otherwise, send as a free text answer
    const answer: BuilderAnswer = { FreeText: text };
    sendAnswer(answer);
  }, [
    inputValue,
    isWaiting,
    hasStarted,
    addUserMessage,
    setMode,
    sendStartBot,
    sendStartSkill,
    sendAnswer,
  ]);

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      if (e.key === "Enter" && !e.shiftKey) {
        e.preventDefault();
        handleSend();
      }
    },
    [handleSend],
  );

  const handleOptionSelect = useCallback(
    (index: number) => {
      if (isWaiting) return;

      // Find the last forge message with options to get the label
      const lastForgeMsg = [...messages]
        .reverse()
        .find(
          (m) =>
            m.role === "forge" &&
            m.turn?.action === "ask_question",
        );

      if (
        lastForgeMsg?.turn?.action === "ask_question" &&
        lastForgeMsg.turn.options[index]
      ) {
        addUserMessage(lastForgeMsg.turn.options[index].label);
      } else {
        addUserMessage(`Option ${index + 1}`);
      }

      const answer: BuilderAnswer = { OptionIndex: index };
      sendAnswer(answer);
    },
    [isWaiting, messages, addUserMessage, sendAnswer],
  );

  const handleAssemble = useCallback(
    (config: BuilderConfig) => {
      sendAssembleBot(config);
      useForgeStore.getState().setWaiting(true);
    },
    [sendAssembleBot],
  );

  const handleCreateSkill = useCallback(() => {
    // Find the last ready_to_assemble turn for skill config
    const lastReady = [...messages]
      .reverse()
      .find(
        (m) =>
          m.role === "forge" &&
          m.turn?.action === "ready_to_assemble",
      );

    if (lastReady?.turn?.action === "ready_to_assemble") {
      const config = lastReady.turn.config;
      const request: SkillRequestDto = {
        name: config.name,
        description: config.description,
        skill_type: "wasm",
        capabilities: config.skills.map((s) => s.name),
      };
      sendCreateSkill(request);
      useForgeStore.getState().setWaiting(true);
    }
  }, [messages, sendCreateSkill]);

  // -----------------------------------------------------------------------
  // Success view (after assembly)
  // -----------------------------------------------------------------------

  const successContent = isAssembled ? (
    <div className="flex flex-col items-center justify-center gap-4 p-6 text-center">
      {assemblyResult && (
        <>
          <div className="text-4xl">&#127881;</div>
          <h3 className="text-lg font-semibold">
            {assemblyResult.bot_name} is ready!
          </h3>
          <p className="text-sm text-muted-foreground">
            Your bot has been created successfully.
          </p>
          <Button
            onClick={() =>
              navigate({
                to: "/bots/$botId",
                params: { botId: assemblyResult.bot_id },
              })
            }
          >
            View Bot
          </Button>
        </>
      )}
      {skillResult && (
        <>
          <div className="text-4xl">&#128295;</div>
          <h3 className="text-lg font-semibold">
            Skill "{skillResult.name}" created!
          </h3>
          <p className="text-sm text-muted-foreground">
            {skillResult.description}
          </p>
          {skillResult.suggested_capabilities.length > 0 && (
            <p className="text-xs text-muted-foreground">
              Capabilities: {skillResult.suggested_capabilities.join(", ")}
            </p>
          )}
        </>
      )}
      <Button
        variant="outline"
        onClick={() => {
          reset();
          const sid = generateSessionId();
          setSessionId(sid);
          setHasStarted(false);
          addForgeMessage({
            action: "clarify",
            message: FORGE_GREETING,
          });
        }}
      >
        Create Another
      </Button>
    </div>
  ) : null;

  // -----------------------------------------------------------------------
  // Render
  // -----------------------------------------------------------------------

  return (
    <div className="flex h-[calc(100vh-3rem)]">
      {/* Chat area */}
      <div className="flex-1 flex flex-col min-w-0">
        {/* Header */}
        <div className="border-b p-4 flex items-center gap-3">
          <div className="flex-shrink-0 w-9 h-9 rounded-full bg-amber-500/20 flex items-center justify-center">
            <Hammer className="w-5 h-5 text-amber-500" />
          </div>
          <div className="flex-1 min-w-0">
            <h2 className="font-semibold text-sm">Forge</h2>
            <p className="text-xs text-muted-foreground">
              {mode === "skill"
                ? "Skill Builder"
                : mode === "bot"
                  ? "Bot Builder"
                  : "Bot & Skill Builder"}
            </p>
          </div>
          <div className="flex items-center gap-1 text-xs text-muted-foreground">
            {isConnected ? (
              <Wifi className="w-3.5 h-3.5 text-green-500" />
            ) : (
              <WifiOff className="w-3.5 h-3.5 text-destructive" />
            )}
          </div>
        </div>

        {/* Messages */}
        {isAssembled && successContent ? (
          successContent
        ) : (
          <>
            <div className="flex-1 overflow-y-auto p-4 space-y-4">
              {messages.map((msg, idx) => (
                <ForgeMessage
                  key={msg.id}
                  message={msg}
                  mode={mode}
                  onOptionSelect={handleOptionSelect}
                  onAssemble={handleAssemble}
                  onCreateSkill={handleCreateSkill}
                  isLast={idx === messages.length - 1}
                  isWaiting={isWaiting}
                />
              ))}
              {isWaiting && <TypingIndicator />}
              <div ref={messagesEndRef} />
            </div>

            {/* Input area */}
            <div className="border-t p-4">
              <div className="flex gap-2">
                <Input
                  value={inputValue}
                  onChange={(e) => setInputValue(e.target.value)}
                  onKeyDown={handleKeyDown}
                  placeholder={
                    hasStarted
                      ? "Type your answer..."
                      : "Describe the bot or skill you want to create..."
                  }
                  disabled={isWaiting || isAssembled}
                />
                <Button
                  onClick={handleSend}
                  disabled={!inputValue.trim() || isWaiting || isAssembled}
                  size="icon"
                >
                  {isWaiting ? (
                    <Loader2 className="w-4 h-4 animate-spin" />
                  ) : (
                    <Send className="w-4 h-4" />
                  )}
                </Button>
              </div>
            </div>
          </>
        )}
      </div>

      {/* Preview panel (desktop only) */}
      <div className="w-80 border-l hidden lg:flex flex-col">
        <div className="p-4 border-b">
          <h3 className="text-sm font-semibold">Preview</h3>
        </div>
        <div className="flex-1 overflow-y-auto p-4">
          <BuilderPreview preview={preview} />
        </div>
      </div>
    </div>
  );
}
