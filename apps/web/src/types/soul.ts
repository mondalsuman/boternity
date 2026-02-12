/**
 * Soul and version types matching the Rust domain model.
 */

export interface Soul {
  bot_id: string;
  content: string;
  version: number;
  created_at: string;
  message: string | null;
}

export interface SoulVersion {
  bot_id: string;
  version: number;
  content: string;
  created_at: string;
  message: string | null;
  hash: string;
}

export interface IdentityFrontmatter {
  model: string;
  temperature: number;
  max_tokens: number;
}

export interface IdentityFile {
  content: string;
  frontmatter: IdentityFrontmatter | null;
}

export interface UserFile {
  content: string;
}
