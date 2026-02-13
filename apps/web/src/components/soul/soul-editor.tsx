import { useCallback, useEffect, useRef, useState } from "react";
import Editor, { type OnMount } from "@monaco-editor/react";
import type { editor as MonacoEditor } from "monaco-editor";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { Switch } from "@/components/ui/switch";
import { Label } from "@/components/ui/label";
import { Badge } from "@/components/ui/badge";
import { Loader2, Save } from "lucide-react";
import { useThemeStore } from "@/stores/theme-store";
import { useDebouncedCallback } from "@/hooks/use-debounce";
import {
  useSoul,
  useUpdateSoul,
  useIdentity,
  useUpdateIdentity,
  useUserContext,
  useUpdateUserContext,
} from "@/hooks/use-soul-queries";
import { MarkdownPreview } from "./markdown-preview";
import { IdentityForm, type IdentityFormValues } from "./identity-form";

/** Auto-save debounce delay (2 seconds of inactivity). */
const AUTOSAVE_DELAY = 2000;

type SoulFileTab = "soul" | "identity" | "user";

import type { SoulVersion } from "@/types/soul";

interface SoulEditorProps {
  botId: string;
  /** When set, opens the diff viewer dialog comparing two versions. */
  diffVersions?: { original: SoulVersion; modified: SoulVersion } | null;
  onCloseDiff?: () => void;
  /** When set, opens the rollback confirmation dialog for this version. */
  rollbackVersion?: SoulVersion | null;
  onCloseRollback?: () => void;
}

/**
 * Monaco editor wrapper for soul files (SOUL.md, IDENTITY.md, USER.md).
 *
 * Features:
 * - File tabs to switch between soul files
 * - Monaco editor with markdown syntax highlighting
 * - Split preview: editor left, rendered markdown right
 * - Auto-save after 2s debounce
 * - Identity form view with toggle to raw text
 */
export function SoulEditor({
  botId,
  diffVersions,
  onCloseDiff,
  rollbackVersion,
  onCloseRollback,
}: SoulEditorProps) {
  const theme = useThemeStore((s) => s.theme);
  const editorRef = useRef<MonacoEditor.IStandaloneCodeEditor | null>(null);

  // Current file tab
  const [activeFile, setActiveFile] = useState<SoulFileTab>("soul");

  // Identity form vs raw toggle
  const [identityFormMode, setIdentityFormMode] = useState(true);

  // Local editor buffers (overridden when data loads)
  const [soulContent, setSoulContent] = useState("");
  const [identityContent, setIdentityContent] = useState("");
  const [userContent, setUserContent] = useState("");

  // Track whether we have loaded initial data
  const soulLoaded = useRef(false);
  const identityLoaded = useRef(false);
  const userLoaded = useRef(false);

  // Queries
  const { data: soulData, isLoading: soulLoading } = useSoul(botId);
  const { data: identityData, isLoading: identityLoading } = useIdentity(botId);
  const { data: userData, isLoading: userLoading } = useUserContext(botId);

  // Mutations
  const updateSoul = useUpdateSoul(botId);
  const updateIdentity = useUpdateIdentity(botId);
  const updateUserContext = useUpdateUserContext(botId);

  // Populate local buffers from fetched data (once)
  useEffect(() => {
    if (soulData && !soulLoaded.current) {
      setSoulContent(soulData.content);
      soulLoaded.current = true;
    }
  }, [soulData]);

  useEffect(() => {
    if (identityData && !identityLoaded.current) {
      setIdentityContent(identityData.raw);
      identityLoaded.current = true;
    }
  }, [identityData]);

  useEffect(() => {
    if (userData && !userLoaded.current) {
      setUserContent(userData.content);
      userLoaded.current = true;
    }
  }, [userData]);

  // Debounced auto-save callbacks
  const [debouncedSaveSoul] = useDebouncedCallback(
    useCallback(
      (content: string) => {
        updateSoul.mutate({ content, message: "Auto-save from web editor" });
      },
      [updateSoul],
    ),
    AUTOSAVE_DELAY,
  );

  const [debouncedSaveIdentity] = useDebouncedCallback(
    useCallback(
      (content: string) => {
        updateIdentity.mutate(content);
      },
      [updateIdentity],
    ),
    AUTOSAVE_DELAY,
  );

  const [debouncedSaveUser] = useDebouncedCallback(
    useCallback(
      (content: string) => {
        updateUserContext.mutate(content);
      },
      [updateUserContext],
    ),
    AUTOSAVE_DELAY,
  );

  // Editor change handler
  const handleEditorChange = useCallback(
    (value: string | undefined) => {
      const content = value ?? "";
      switch (activeFile) {
        case "soul":
          setSoulContent(content);
          debouncedSaveSoul(content);
          break;
        case "identity":
          setIdentityContent(content);
          debouncedSaveIdentity(content);
          break;
        case "user":
          setUserContent(content);
          debouncedSaveUser(content);
          break;
      }
    },
    [activeFile, debouncedSaveSoul, debouncedSaveIdentity, debouncedSaveUser],
  );

  // Identity form change handler
  const handleIdentityFormChange = useCallback(
    (rawContent: string) => {
      setIdentityContent(rawContent);
      debouncedSaveIdentity(rawContent);
    },
    [debouncedSaveIdentity],
  );

  // Derive active content and loading state
  const activeContent =
    activeFile === "soul"
      ? soulContent
      : activeFile === "identity"
        ? identityContent
        : userContent;

  const isFileLoading =
    activeFile === "soul"
      ? soulLoading
      : activeFile === "identity"
        ? identityLoading
        : userLoading;

  const isSaving =
    activeFile === "soul"
      ? updateSoul.isPending
      : activeFile === "identity"
        ? updateIdentity.isPending
        : updateUserContext.isPending;

  // Monaco theme follows app theme
  const monacoTheme =
    theme === "light" ? "light" : "vs-dark";

  const handleEditorMount: OnMount = (editor) => {
    editorRef.current = editor;
  };

  // Derive identity form values from parsed data or defaults
  const identityFormValues: IdentityFormValues = identityData?.parsed
    ? {
        model: identityData.parsed.model ?? "claude-sonnet-4-20250514",
        temperature: identityData.parsed.temperature ?? 0.7,
        max_tokens: identityData.parsed.max_tokens ?? 4096,
      }
    : { model: "claude-sonnet-4-20250514", temperature: 0.7, max_tokens: 4096 };

  return (
    <div className="space-y-3">
      {/* File tabs */}
      <div className="flex items-center justify-between">
        <Tabs
          value={activeFile}
          onValueChange={(v) => setActiveFile(v as SoulFileTab)}
        >
          <TabsList>
            <TabsTrigger value="soul">SOUL.md</TabsTrigger>
            <TabsTrigger value="identity">IDENTITY.md</TabsTrigger>
            <TabsTrigger value="user">USER.md</TabsTrigger>
          </TabsList>
        </Tabs>

        <div className="flex items-center gap-3">
          {/* Save indicator */}
          {isSaving && (
            <Badge variant="outline" className="gap-1.5 text-muted-foreground">
              <Loader2 className="size-3 animate-spin" />
              Saving
            </Badge>
          )}
          {!isSaving && !isFileLoading && (
            <Badge variant="outline" className="gap-1.5 text-muted-foreground">
              <Save className="size-3" />
              Auto-save
            </Badge>
          )}

          {/* Identity form/raw toggle */}
          {activeFile === "identity" && (
            <div className="flex items-center gap-2">
              <Label htmlFor="identity-form-toggle" className="text-xs">
                Form view
              </Label>
              <Switch
                id="identity-form-toggle"
                checked={identityFormMode}
                onCheckedChange={setIdentityFormMode}
              />
            </div>
          )}
        </div>
      </div>

      {/* Editor + Preview split */}
      <div className="grid md:grid-cols-2 gap-4 min-h-[500px]">
        {/* Left: Editor or Form */}
        <div className="rounded-lg border overflow-hidden bg-background">
          {isFileLoading ? (
            <div className="flex items-center justify-center h-full min-h-[500px] text-muted-foreground">
              <Loader2 className="size-5 animate-spin mr-2" />
              Loading...
            </div>
          ) : activeFile === "identity" && identityFormMode ? (
            <IdentityForm
              values={identityFormValues}
              rawContent={identityContent}
              onChange={handleIdentityFormChange}
            />
          ) : (
            <Editor
              height="500px"
              language="markdown"
              theme={monacoTheme}
              value={activeContent}
              onChange={handleEditorChange}
              onMount={handleEditorMount}
              options={{
                minimap: { enabled: false },
                wordWrap: "on",
                lineNumbers: "on",
                fontSize: 14,
                scrollBeyondLastLine: false,
                padding: { top: 12, bottom: 12 },
                automaticLayout: true,
              }}
            />
          )}
        </div>

        {/* Right: Markdown Preview */}
        <div className="rounded-lg border overflow-auto bg-background min-h-[500px]">
          <div className="sticky top-0 bg-background border-b px-4 py-2">
            <span className="text-xs font-medium text-muted-foreground uppercase tracking-wider">
              Preview
            </span>
          </div>
          <MarkdownPreview content={activeContent} />
        </div>
      </div>
    </div>
  );
}
