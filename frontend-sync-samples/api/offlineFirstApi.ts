/**
 * Portfolio sample — offline-first API edge.
 * Highlights: GET falls back to Dexie cache; mutations enqueue Outbox on failure
 * (UI never freezes waiting for the network).
 * Source: adapted from `src/lib/api.ts` (`apiGet` / `apiMutateJson`).
 */

import { enqueueOutbox } from "../dexie/offlineDb";
import { authHeaders, getApiBase } from "../sync/syncPolicy";

function apiUrl(path: string): string {
  return `${getApiBase()}${path}`;
}

/** Prefer live Axum; on failure return the provided Dexie/cache fallback. */
export async function apiGet<T>(path: string, fallback: T): Promise<T> {
  try {
    const res = await fetch(apiUrl(path), {
      headers: authHeaders(),
    });
    if (!res.ok) throw new Error(`HTTP ${res.status}`);
    return (await res.json()) as T;
  } catch {
    return fallback;
  }
}

/**
 * Returns parsed JSON on success; `null` when offline (mutation queued).
 * Callers update optimistic UI / Dexie themselves when `null` is returned.
 */
export async function apiMutateJson<T>(
  method: string,
  path: string,
  body?: unknown,
): Promise<T | null> {
  try {
    const res = await fetch(apiUrl(path), {
      method,
      headers: authHeaders({ "Content-Type": "application/json" }),
      body: body === undefined ? undefined : JSON.stringify(body),
    });
    if (!res.ok) throw new Error(`HTTP ${res.status}`);
    if (res.status === 204) return null;
    return (await res.json()) as T;
  } catch {
    await enqueueOutbox({ method, path, body });
    return null;
  }
}
