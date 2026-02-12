/**
 * Bot types matching boternity-types domain model.
 */

export type BotStatus = "active" | "disabled" | "archived";

export interface Bot {
  id: string;
  name: string;
  slug: string;
  description: string | null;
  status: BotStatus;
  emoji: string | null;
  category: string | null;
  created_at: string;
  updated_at: string;
  version_count: number;
}

export interface CreateBotRequest {
  name: string;
  description?: string;
  emoji?: string;
  category?: string;
}

export interface UpdateBotRequest {
  name?: string;
  description?: string | null;
  status?: BotStatus;
  emoji?: string | null;
  category?: string | null;
}
