/**
 * Chat session and message types matching the Rust domain model.
 */

export type SessionStatus = "active" | "completed";

export interface ChatSession {
  id: string;
  bot_id: string;
  title: string | null;
  started_at: string;
  ended_at: string | null;
  total_input_tokens: number;
  total_output_tokens: number;
  message_count: number;
  model: string;
  status: SessionStatus;
}

export type MessageRole = "user" | "assistant";

export interface ChatMessage {
  id: string;
  session_id: string;
  role: MessageRole;
  content: string;
  created_at: string;
  input_tokens: number | null;
  output_tokens: number | null;
  model: string | null;
}

export interface ChatStreamRequest {
  session_id?: string;
  message: string;
}

export interface StreamEvent {
  type: "session" | "text_delta" | "usage" | "done" | "error";
  session_id?: string;
  text?: string;
  message?: string;
  input_tokens?: number;
  output_tokens?: number;
}
