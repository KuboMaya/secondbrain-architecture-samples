/**
 * Portfolio sample — runtime API endpoint + Bearer policy for Tailscale peers.
 * Highlights: same-origin `*.ts.net` detection, loopback vs remote auth.
 * Source: adapted from `src/lib/syncPolicy.ts` (no secrets; env var names only).
 */

const MULTI_KEY = "secondbrain.multi_device_sync";
const API_BASE_KEY = "secondbrain.api_base";
const AUTH_TOKEN_KEY = "secondbrain.sync_auth_token";

/** Placeholder — Tailscale CGNAT style peer (not a real host). */
const DEFAULT_API_BASE = "http://100.x.x.x:3000/api";

function envApiBase(): string {
  try {
    const v = (import.meta.env?.VITE_API_BASE as string | undefined)?.trim();
    if (v) return v.replace(/\/+$/, "");
  } catch {
    // ignore
  }
  return DEFAULT_API_BASE;
}

function envAuthToken(): string {
  try {
    return ((import.meta.env?.VITE_SYNC_AUTH_TOKEN as string | undefined) || "").trim();
  } catch {
    return "";
  }
}

/** Opened through Tailscale Serve HTTPS → use same origin `/api`. */
export function sameOriginTailscaleApiBase(): string | null {
  try {
    if (typeof window === "undefined") return null;
    const host = window.location.hostname.toLowerCase();
    if (!host.endsWith(".ts.net")) return null;
    return `${window.location.origin}/api`;
  } catch {
    return null;
  }
}

export function isMultiDeviceSyncEnabled(): boolean {
  try {
    return localStorage.getItem(MULTI_KEY) === "1";
  } catch {
    return false;
  }
}

/** Runtime or build-time Axum API base (no trailing slash). */
export function getApiBase(): string {
  const same = sameOriginTailscaleApiBase();
  if (same) return same;
  try {
    const stored = localStorage.getItem(API_BASE_KEY)?.trim();
    if (stored) return stored.replace(/\/+$/, "");
  } catch {
    // ignore
  }
  return envApiBase();
}

/** Bearer token for Tailscale peers (loopback Axum does not require it). */
export function getSyncAuthToken(): string {
  try {
    const stored = localStorage.getItem(AUTH_TOKEN_KEY)?.trim();
    if (stored) return stored;
  } catch {
    // ignore
  }
  return envAuthToken();
}

export function authHeaders(
  extra?: Record<string, string>,
): Record<string, string> {
  const headers: Record<string, string> = { ...(extra || {}) };
  const token = getSyncAuthToken();
  if (token) {
    headers.Authorization = `Bearer ${token}`;
  }
  return headers;
}
