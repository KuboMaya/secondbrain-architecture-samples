/**
 * Portfolio sample — Dexie offline cache + Outbox.
 * Highlights: versioned IndexedDB schema, navigator.storage.persist(),
 * enqueue on network failure, ordered flush to Tailscale/local Axum.
 * Source: adapted from `src/lib/dexie/db.ts` (Cloudflare-era comments removed).
 */

import Dexie, { type EntityTable } from "dexie";

/** Offline cache of tasks mirrored from the backend / sync layer. */
export type CachedTask = {
  id: string;
  title: string;
  status: string;
  priority: number;
  due_date?: string | null;
  parent_id?: string | null;
  tags?: string;
  reward_multiplier?: number;
  updatedAt: string;
  version: number;
  deletedAt?: string | null;
};

/** Mutations queued while offline (flushed when API is reachable). */
export type OutboxItem = {
  id?: number;
  method: string;
  path: string;
  body?: string;
  createdAt: string;
};

/**
 * PWA / offline-first IndexedDB schema (Dexie).
 * Survives reloads when `navigator.storage.persist()` is granted.
 *
 * (Full app also caches schedules, knowledge, inventory, sleep, reward — same pattern.)
 */
class SecondBrainDb extends Dexie {
  tasks!: EntityTable<CachedTask, "id">;
  outbox!: EntityTable<OutboxItem, "id">;

  constructor() {
    super("second_brain");
    this.version(1).stores({
      tasks: "id, status, updatedAt, version",
      outbox: "++id, createdAt, path",
    });
  }
}

export const localDb = new SecondBrainDb();

/** Request durable storage so Dexie data is not evicted under pressure. */
export async function ensurePersistentStorage(): Promise<boolean> {
  if (!("storage" in navigator) || typeof navigator.storage?.persist !== "function") {
    return false;
  }
  try {
    if (typeof navigator.storage.persisted === "function") {
      const already = await navigator.storage.persisted();
      if (already) return true;
    }
    return await navigator.storage.persist();
  } catch {
    return false;
  }
}

export async function initOfflineStorage(): Promise<{ persisted: boolean }> {
  await localDb.open();
  const persisted = await ensurePersistentStorage();
  return { persisted };
}

/** Enqueue an offline mutation (Outbox pattern). */
export async function enqueueOutbox(input: {
  method: string;
  path: string;
  body?: unknown;
}): Promise<number> {
  const id = await localDb.outbox.add({
    method: input.method,
    path: input.path,
    body:
      input.body === undefined
        ? undefined
        : typeof input.body === "string"
          ? input.body
          : JSON.stringify(input.body),
    createdAt: new Date().toISOString(),
  });
  return Number(id);
}

/**
 * Flush queued mutations in createdAt order.
 * Stops on first failure so ordering / partial apply stays predictable.
 */
export async function flushOutbox(
  apiBase: string,
  authHeaders: () => Record<string, string> = () => ({}),
): Promise<{ flushed: number; remaining: number }> {
  const items = await localDb.outbox.orderBy("createdAt").toArray();
  let flushed = 0;
  for (const item of items) {
    if (item.id == null) continue;
    try {
      const res = await fetch(`${apiBase}${item.path}`, {
        method: item.method,
        headers: { "Content-Type": "application/json", ...authHeaders() },
        body: item.body,
      });
      if (!res.ok) break;
      await localDb.outbox.delete(item.id);
      flushed += 1;
    } catch {
      break;
    }
  }
  const remaining = await localDb.outbox.count();
  return { flushed, remaining };
}

/** Periodic Outbox flush + online event hook (default 15s). */
export function startOutboxFlushLoop(
  flush: () => void,
  intervalMs = 15_000,
): () => void {
  const tick = () => flush();
  tick();
  const id = window.setInterval(tick, intervalMs);
  const onOnline = () => tick();
  window.addEventListener("online", onOnline);
  return () => {
    window.clearInterval(id);
    window.removeEventListener("online", onOnline);
  };
}
