/**
 * Portfolio sample — full Dexie schema + Outbox (persist / enqueue / flush / loop).
 * Source: src/lib/dexie/db.ts (Cloudflare-era notes removed; no secrets).
 */

import Dexie, { type EntityTable } from "dexie";
import { authHeaders, getApiBase } from "../syncPolicy";

/** Offline cache of tasks mirrored from the backend / sync queue. */
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

export type CachedSchedule = {
  id: string;
  title: string;
  start_time: string;
  end_time: string;
  is_all_day: number;
  recurrence_rule: string | null;
  location: string | null;
  notes: string | null;
  updatedAt: string;
  version: number;
  deletedAt?: string | null;
};

/** Full dictionary entry fields for offline Knowledge UI. */
export type CachedKnowledge = {
  id: string;
  index_id: string;
  keyword: string;
  content: string;
  source_url?: string | null;
  tags: string;
  ai_summary?: string | null;
  english_content?: string | null;
  createdAt: string;
  updatedAt: string;
  version: number;
  deletedAt?: string | null;
};

export type CachedDictionaryIndex = {
  id: string;
  name: string;
  parent_id: string | null;
  createdAt: string;
  updatedAt: string;
  version: number;
  deletedAt?: string | null;
};

export type CachedInventoryCategory = {
  id: string;
  name: string;
  parent_id: string | null;
  createdAt: string;
  updatedAt: string;
  version: number;
  deletedAt?: string | null;
};

export type CachedInventoryItem = {
  id: string;
  category_id: string;
  item_name: string;
  status: string;
  remaining_amount: number;
  unit?: string | null;
  notes?: string | null;
  last_purchased?: string | null;
  createdAt: string;
  updatedAt: string;
  version: number;
  deletedAt?: string | null;
};

export type CachedSleepRecord = {
  id: string;
  target_date: string;
  alarm_time?: string | null;
  is_alarm_active: number;
  sleep_start?: string | null;
  sleep_end?: string | null;
  createdAt: string;
  updatedAt: string;
  version: number;
  deletedAt?: string | null;
};

export type CachedMorningConfig = {
  id: string;
  is_enabled: number;
  voiceroid_path?: string | null;
  weather_location?: string | null;
  read_aloud_calendar: number;
  createdAt: string;
  updatedAt: string;
  version: number;
  deletedAt?: string | null;
};

export type CachedRewardState = {
  id: string;
  hidden_points: number;
  monthly_budget: number;
  createdAt: string;
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
 */
class SecondBrainDb extends Dexie {
  tasks!: EntityTable<CachedTask, "id">;
  schedules!: EntityTable<CachedSchedule, "id">;
  knowledge!: EntityTable<CachedKnowledge, "id">;
  knowledgeIndexes!: EntityTable<CachedDictionaryIndex, "id">;
  inventoryCategories!: EntityTable<CachedInventoryCategory, "id">;
  inventoryItems!: EntityTable<CachedInventoryItem, "id">;
  sleepRecords!: EntityTable<CachedSleepRecord, "id">;
  morningConfig!: EntityTable<CachedMorningConfig, "id">;
  rewardState!: EntityTable<CachedRewardState, "id">;
  outbox!: EntityTable<OutboxItem, "id">;

  constructor() {
    super("second_brain");
    this.version(1).stores({
      tasks: "id, status, updatedAt, version",
      knowledge: "id, keyword, updatedAt",
      outbox: "++id, createdAt, path",
    });
    this.version(2).stores({
      tasks: "id, status, updatedAt, version",
      schedules: "id, start_time, updatedAt, version",
      knowledge: "id, keyword, updatedAt",
      outbox: "++id, createdAt, path",
    });
    // v3: full knowledge fields + indexes + inventory offline caches.
    this.version(3).stores({
      tasks: "id, status, updatedAt, version",
      schedules: "id, start_time, updatedAt, version",
      knowledge: "id, index_id, keyword, updatedAt, version",
      knowledgeIndexes: "id, parent_id, updatedAt, version",
      inventoryCategories: "id, name, updatedAt, version",
      inventoryItems: "id, category_id, status, updatedAt, version",
      outbox: "++id, createdAt, path",
    });
    // v4: sleep + morning routine config (PWA alarm / location).
    this.version(4).stores({
      tasks: "id, status, updatedAt, version",
      schedules: "id, start_time, updatedAt, version",
      knowledge: "id, index_id, keyword, updatedAt, version",
      knowledgeIndexes: "id, parent_id, updatedAt, version",
      inventoryCategories: "id, name, updatedAt, version",
      inventoryItems: "id, category_id, status, updatedAt, version",
      sleepRecords: "id, target_date, updatedAt, version",
      morningConfig: "id, updatedAt, version",
      outbox: "++id, createdAt, path",
    });
    // v5: reward_state offline cache (synced via SyncRecord / PWA CryptoKey).
    this.version(5).stores({
      tasks: "id, status, updatedAt, version",
      schedules: "id, start_time, updatedAt, version",
      knowledge: "id, index_id, keyword, updatedAt, version",
      knowledgeIndexes: "id, parent_id, updatedAt, version",
      inventoryCategories: "id, name, updatedAt, version",
      inventoryItems: "id, category_id, status, updatedAt, version",
      sleepRecords: "id, target_date, updatedAt, version",
      morningConfig: "id, updatedAt, version",
      rewardState: "id, updatedAt, version",
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
  // Architecture: request durable storage before relying on Dexie caches / outbox.
  const persisted = await ensurePersistentStorage();
  return { persisted };
}

/**
 * Enqueue an offline mutation (Outbox pattern).
 * Used by apiMutateJson on network failure and by UI/sync mocks.
 */
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

/** Explicit mock/demo helper for enqueue (same store as production outbox). */
export async function enqueueOutboxMock(
  method: string,
  path: string,
  body?: unknown,
): Promise<{ queued: true; id: number }> {
  const id = await enqueueOutbox({ method, path, body });
  return { queued: true, id };
}

/** Flush queued mutations when the backend is reachable again. */
export async function flushOutbox(
  apiBase: string = getApiBase(),
): Promise<{ flushed: number; remaining: number }> {
  const items = await localDb.outbox.orderBy("createdAt").toArray();
  let flushed = 0;
  for (const item of items) {
    if (item.id == null) continue;
    try {
      const res = await fetch(`${apiBase}${item.path}`, {
        method: item.method,
        headers: authHeaders({ "Content-Type": "application/json" }),
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

/** Flush Outbox to Tailscale/local Axum. */
export async function flushOutboxAndSync(
  apiBase: string = getApiBase(),
): Promise<{ flushed: number; remaining: number }> {
  return flushOutbox(apiBase);
}

/** Periodic Outbox flush to Axum (default 15s). */
export function startOutboxFlushLoop(intervalMs = 15_000): () => void {
  const tick = () => {
    void flushOutboxAndSync();
  };
  tick();
  const id = window.setInterval(tick, intervalMs);
  const onOnline = () => tick();
  window.addEventListener("online", onOnline);
  return () => {
    window.clearInterval(id);
    window.removeEventListener("online", onOnline);
  };
}
