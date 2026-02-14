/**
 * Skill system types matching boternity-types skill domain model.
 */

export type TrustTier = "local" | "verified" | "untrusted";
export type SkillType = "prompt" | "tool";

export type Capability =
  | "read_file"
  | "write_file"
  | "http_get"
  | "http_post"
  | "exec_command"
  | "read_env"
  | "recall_memory"
  | "get_secret";

export interface SkillMetadata {
  author?: string;
  version?: string;
  skill_type?: SkillType;
  capabilities?: Capability[];
  dependencies?: string[];
  conflicts_with?: string[];
  trust_tier?: TrustTier;
  parents?: string[];
  secrets?: string[];
  categories?: string[];
}

export interface SkillManifest {
  name: string;
  description: string;
  license?: string;
  compatibility?: string;
  metadata?: SkillMetadata;
  allowed_tools?: string;
}

/** An installed skill in the global library. */
export interface InstalledSkill {
  name: string;
  description: string;
  skill_type: SkillType | null;
  trust_tier: TrustTier | null;
  version: string | null;
  source: unknown;
  installed: boolean;
}

/** A skill attached to a specific bot. */
export interface BotSkillConfig {
  name: string;
  description: string;
  skill_type: SkillType | null;
  trust_tier: TrustTier | null;
  enabled: boolean;
  overrides: Record<string, string>;
}

/** Skill detail with resolved capabilities. */
export interface SkillDetail {
  manifest: SkillManifest;
  body: string;
  resolved_capabilities: string[];
  parent_chain: string[];
  conflicts_with: string[];
}

/** A skill discovered from a registry (not yet installed). */
export interface DiscoveredSkill {
  name: string;
  description: string;
  source: string;
  categories: string[];
  install_count: number | null;
  trust_tier: TrustTier | null;
}
