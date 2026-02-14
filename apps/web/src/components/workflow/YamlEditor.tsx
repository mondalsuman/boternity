/**
 * YAML editor panel for the workflow builder.
 *
 * Uses Monaco Editor with YAML language mode. Changes are debounced (500ms)
 * and validated inline before propagating to the parent.
 */

import { useCallback, useRef, useState } from "react";
import Editor, { type OnMount } from "@monaco-editor/react";
import type { editor } from "monaco-editor";
import { AlertCircle } from "lucide-react";

import { useDebouncedCallback } from "@/hooks/use-debounce";

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

interface YamlEditorProps {
  /** Current YAML string value. */
  value: string;
  /** Called with the new YAML string when the user edits (debounced 500ms). */
  onChange: (yaml: string) => void;
  /** Whether the editor is read-only. */
  readOnly?: boolean;
}

// ---------------------------------------------------------------------------
// Simple YAML validation (structural check)
// ---------------------------------------------------------------------------

function validateYaml(yaml: string): string | null {
  const trimmed = yaml.trim();
  if (!trimmed) return "YAML is empty";

  // Check for basic YAML structure issues
  const lines = trimmed.split("\n");
  for (let i = 0; i < lines.length; i++) {
    const line = lines[i];
    // Check for tabs (YAML uses spaces only)
    if (line.includes("\t")) {
      return `Line ${i + 1}: Tabs are not allowed in YAML, use spaces`;
    }
  }

  // Must contain at least 'name:' and 'steps:' keys
  if (!trimmed.includes("name:")) {
    return "Missing required field: name";
  }
  if (!trimmed.includes("steps:")) {
    return "Missing required field: steps";
  }

  return null;
}

// ---------------------------------------------------------------------------
// YamlEditor Component
// ---------------------------------------------------------------------------

export function YamlEditor({ value, onChange, readOnly = false }: YamlEditorProps) {
  const editorRef = useRef<editor.IStandaloneCodeEditor | null>(null);
  const [validationError, setValidationError] = useState<string | null>(null);

  const [debouncedOnChange] = useDebouncedCallback(
    (newValue: string) => {
      const error = validateYaml(newValue);
      setValidationError(error);

      if (!error) {
        onChange(newValue);
      }
    },
    500,
  );

  const handleEditorMount: OnMount = useCallback(
    (editor) => {
      editorRef.current = editor;

      // Configure editor options
      editor.updateOptions({
        minimap: { enabled: false },
        lineNumbers: "on",
        scrollBeyondLastLine: false,
        wordWrap: "on",
        tabSize: 2,
        insertSpaces: true,
        readOnly,
        fontSize: 13,
        fontFamily: "'JetBrains Mono', 'Fira Code', 'Cascadia Code', monospace",
      });
    },
    [readOnly],
  );

  const handleChange = useCallback(
    (newValue: string | undefined) => {
      if (newValue !== undefined) {
        debouncedOnChange(newValue);
      }
    },
    [debouncedOnChange],
  );

  return (
    <div className="flex flex-col h-full">
      {/* Validation bar */}
      {validationError && (
        <div className="flex items-center gap-2 px-3 py-1.5 bg-destructive/10 border-b text-destructive text-xs shrink-0">
          <AlertCircle className="size-3.5 shrink-0" />
          <span className="truncate">{validationError}</span>
        </div>
      )}

      {/* Editor */}
      <div className="flex-1 min-h-0">
        <Editor
          defaultLanguage="yaml"
          value={value}
          onChange={handleChange}
          onMount={handleEditorMount}
          theme="vs-dark"
          options={{
            minimap: { enabled: false },
            lineNumbers: "on",
            scrollBeyondLastLine: false,
            wordWrap: "on",
            tabSize: 2,
            insertSpaces: true,
            readOnly,
            fontSize: 13,
          }}
        />
      </div>
    </div>
  );
}
