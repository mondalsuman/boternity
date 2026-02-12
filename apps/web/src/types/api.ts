/**
 * API envelope types matching `crates/boternity-api/src/http/response.rs`.
 *
 * Every response from the Rust backend is wrapped in this envelope format.
 */

export interface ApiMeta {
  request_id: string;
  timestamp: string;
  response_time_ms: number;
}

export interface ApiErrorDetail {
  code: string;
  message: string;
  details?: unknown;
}

export interface ApiEnvelope<T> {
  data?: T;
  meta: ApiMeta;
  errors?: ApiErrorDetail[];
  _links?: Record<string, string>;
}

/**
 * Paginated list wrapper returned by list endpoints.
 */
export interface PaginatedList<T> {
  items: T[];
  total: number;
  offset: number;
  limit: number;
}
