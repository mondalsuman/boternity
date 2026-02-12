import { useCallback, useEffect, useState } from "react";
import { Label } from "@/components/ui/label";
import { Input } from "@/components/ui/input";
import { Slider } from "@/components/ui/slider";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";

/**
 * Known model options for the identity dropdown.
 * Matches models available via Anthropic / OpenAI / Bedrock providers.
 */
const MODEL_OPTIONS = [
  { value: "claude-sonnet-4-20250514", label: "Claude Sonnet 4" },
  { value: "claude-opus-4-20250514", label: "Claude Opus 4" },
  { value: "claude-haiku-3-5-20241022", label: "Claude 3.5 Haiku" },
  { value: "gpt-4o", label: "GPT-4o" },
  { value: "gpt-4o-mini", label: "GPT-4o Mini" },
] as const;

/** Parsed identity frontmatter values for the form. */
export interface IdentityFormValues {
  model: string;
  temperature: number;
  max_tokens: number;
}

interface IdentityFormProps {
  /** Current parsed frontmatter values. */
  values: IdentityFormValues;
  /** Called when any field changes. Receives the full raw IDENTITY.md string. */
  onChange: (rawContent: string) => void;
  /** Current raw IDENTITY.md content (used to preserve non-frontmatter body). */
  rawContent: string;
}

/**
 * Parse frontmatter values and body from raw IDENTITY.md content.
 * Simple line-based YAML parser matching the Rust implementation.
 */
function parseFrontmatter(raw: string): {
  fields: Record<string, string>;
  body: string;
} {
  const lines = raw.split("\n");
  const fields: Record<string, string> = {};
  let body = "";
  let inFrontmatter = false;
  let frontmatterEnd = 0;

  for (let i = 0; i < lines.length; i++) {
    const line = lines[i].trim();
    if (i === 0 && line === "---") {
      inFrontmatter = true;
      continue;
    }
    if (inFrontmatter && line === "---") {
      inFrontmatter = false;
      frontmatterEnd = i + 1;
      continue;
    }
    if (inFrontmatter) {
      const colonIdx = line.indexOf(":");
      if (colonIdx > 0) {
        const key = line.slice(0, colonIdx).trim();
        const value = line.slice(colonIdx + 1).trim();
        fields[key] = value;
      }
    }
  }

  body = lines.slice(frontmatterEnd).join("\n");
  return { fields, body };
}

/**
 * Rebuild the raw IDENTITY.md content from form values + body.
 */
function buildRawContent(values: IdentityFormValues, body: string): string {
  const fm = [
    "---",
    `model: ${values.model}`,
    `temperature: ${values.temperature}`,
    `max_tokens: ${values.max_tokens}`,
    "---",
  ].join("\n");

  return body.trim() ? `${fm}\n${body}` : `${fm}\n`;
}

/**
 * Form view for IDENTITY.md with model dropdown, temperature slider,
 * and max_tokens input. Changes rebuild the raw content and call onChange.
 */
export function IdentityForm({ values, onChange, rawContent }: IdentityFormProps) {
  const [model, setModel] = useState(values.model);
  const [temperature, setTemperature] = useState(values.temperature);
  const [maxTokens, setMaxTokens] = useState(values.max_tokens);

  // Sync internal state when external values change (e.g., after fetch)
  useEffect(() => {
    setModel(values.model);
    setTemperature(values.temperature);
    setMaxTokens(values.max_tokens);
  }, [values.model, values.temperature, values.max_tokens]);

  const { body } = parseFrontmatter(rawContent);

  const emitChange = useCallback(
    (overrides: Partial<IdentityFormValues>) => {
      const newValues = {
        model: overrides.model ?? model,
        temperature: overrides.temperature ?? temperature,
        max_tokens: overrides.max_tokens ?? maxTokens,
      };
      onChange(buildRawContent(newValues, body));
    },
    [model, temperature, maxTokens, body, onChange],
  );

  return (
    <div className="space-y-6 p-4">
      {/* Model */}
      <div className="space-y-2">
        <Label htmlFor="identity-model">Model</Label>
        <Select
          value={model}
          onValueChange={(v) => {
            setModel(v);
            emitChange({ model: v });
          }}
        >
          <SelectTrigger id="identity-model">
            <SelectValue placeholder="Select a model" />
          </SelectTrigger>
          <SelectContent>
            {MODEL_OPTIONS.map((opt) => (
              <SelectItem key={opt.value} value={opt.value}>
                {opt.label}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
      </div>

      {/* Temperature */}
      <div className="space-y-2">
        <div className="flex items-center justify-between">
          <Label htmlFor="identity-temperature">Temperature</Label>
          <span className="text-sm text-muted-foreground tabular-nums">
            {temperature.toFixed(2)}
          </span>
        </div>
        <Slider
          id="identity-temperature"
          min={0}
          max={2}
          step={0.01}
          value={[temperature]}
          onValueChange={([v]) => {
            setTemperature(v);
            emitChange({ temperature: v });
          }}
        />
        <p className="text-xs text-muted-foreground">
          Lower values produce more focused output. Higher values increase creativity.
        </p>
      </div>

      {/* Max Tokens */}
      <div className="space-y-2">
        <Label htmlFor="identity-max-tokens">Max Tokens</Label>
        <Input
          id="identity-max-tokens"
          type="number"
          min={1}
          max={200000}
          value={maxTokens}
          onChange={(e) => {
            const v = Math.max(1, parseInt(e.target.value, 10) || 1);
            setMaxTokens(v);
            emitChange({ max_tokens: v });
          }}
        />
        <p className="text-xs text-muted-foreground">
          Maximum number of tokens in the model response. Default: 4096.
        </p>
      </div>
    </div>
  );
}
