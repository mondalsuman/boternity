/**
 * Typed fetch wrapper that unwraps the ApiResponse envelope from the Rust backend.
 *
 * All REST API calls go through this client. It handles:
 * - Base URL prefixing (/api/v1)
 * - JSON content-type header
 * - Envelope unwrapping (extracts data from { data, meta, errors })
 * - Error extraction (throws ApiError with code from envelope errors)
 */

import type { ApiEnvelope } from "@/types/api";

export class ApiError extends Error {
  public readonly code: string;
  public readonly details?: unknown;

  constructor(code: string, message: string, details?: unknown) {
    super(message);
    this.name = "ApiError";
    this.code = code;
    this.details = details;
  }
}

/**
 * Fetch wrapper that calls the backend API and unwraps the envelope.
 *
 * @param path - API path relative to /api/v1 (e.g., "/bots", "/bots/123")
 * @param init - Standard RequestInit options
 * @returns The unwrapped data from the envelope
 * @throws ApiError if the envelope contains errors
 */
export async function apiFetch<T>(
  path: string,
  init?: RequestInit,
): Promise<T> {
  const res = await fetch(`/api/v1${path}`, {
    ...init,
    headers: {
      "Content-Type": "application/json",
      ...init?.headers,
    },
  });

  // Handle non-JSON responses (e.g., 502 from proxy)
  const contentType = res.headers.get("content-type");
  if (!contentType?.includes("application/json")) {
    throw new ApiError(
      "NETWORK_ERROR",
      `Server returned ${res.status}: ${res.statusText}`,
    );
  }

  const envelope: ApiEnvelope<T> = await res.json();

  if (envelope.errors && envelope.errors.length > 0) {
    const first = envelope.errors[0];
    throw new ApiError(first.code, first.message, first.details);
  }

  return envelope.data as T;
}
